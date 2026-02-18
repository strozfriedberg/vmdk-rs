use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    sync::{Arc, Mutex}
};
use tokio::runtime::Runtime;
use tracing::info;
use url::Url;

use crate::{
    cache::Cache,
    cachereadseek::CacheReadSeek,
    errors::{OpenError, OpenErrorKind},
    extent_description::{ExtentDescription, ExtentDescriptionInner},
    header::{VmdkSparseMeta, VmdkSeSparseMeta, read_header_sparse, read_header_sesparse},
    vmdk_reader::source_for_url,
    readseek::ReadSeek,
    storage::{ExtentStorage, FlatStorage, SparseStorage}
};

/*
RW 8323072 FLAT "CentOS 3-f001.vmdk" 0
RW 2162688 FLAT "CentOS 3-f002.vmdk" 0

sector_start = 0, sectors = 8323072
sector_start = 8323072, sectors = 2162688
*/

#[derive(Debug)]
pub struct Extent {
    pub start_sector: u64,
    pub sectors: u64,
    pub storage: ExtentStorage
}

impl Extent {
    pub fn spans(&self) -> impl Iterator<Item = (u64, u64)> {
        match &self.storage {
            // Sparse storage is a collection of blocks of bytes.
            // It need not cover the extent's whole space.
            ExtentStorage::Sparse(storage) => storage.grain_table.keys()
                .map(|goff| {
                    // grain_size is in sectors
                    let beg = self.start_sector + goff * storage.grain_size;
                    let end = beg + storage.grain_size;
                    (beg, end)
                })
                .collect::<Vec<_>>(),
            // Flat and Zero storage are each a single block of bytes.
            ExtentStorage::Flat(_) | ExtentStorage::Zero =>
                vec![(self.start_sector, self.start_sector + self.sectors)]
        }.into_iter()
    }

    pub fn has_file(&self) -> bool {
        !matches!(self.storage, ExtentStorage::Zero)
    }
}

const SECTOR_SIZE: u64 = 512;

fn read_grain_table_sparse<R>(
    h: &VmdkSparseMeta,
    src: &mut R
) -> Result<HashMap<u64, u64>, std::io::Error>
where
    R: Read + Seek
{
    // h.sectors: number of sectors in the extent
    // h.cluster_sectors: number of sectors per cluster
    // h.l1_offset: offset of l1 grain directory
    // h.l1_len: number of l1 grain directory entries
    // h.l2_len: number of l2 grain table entries
    //      (NB: last l2 group may be smaller)

    // read level 1
    src.seek(SeekFrom::Start(h.l1_offset))?;

    let l1_entries = (0..h.l1_len)
        .map(|_| src.read_u32::<LittleEndian>().map(|e| e as u64 * SECTOR_SIZE))
        .collect::<Result<Vec<u64>, std::io::Error>>()?;

    // read level 2
    let mut grain_table = HashMap::new();
    let mut start_cluster = 0;
    let total_clusters = h.sectors / h.cluster_sectors;

    for l2_offset in l1_entries {
        if start_cluster == total_clusters {
            // we've exhausted all the clusters; stop
            break;
        }

        let l2_len = h.l2_len.min(total_clusters - start_cluster);

        if l2_offset == 0 {
            // the data for this entry is in the parent
            start_cluster += l2_len;
            continue;
        }

        src.seek(SeekFrom::Start(l2_offset))?;

        let l2_entries = (0..l2_len)
            .map(|_| src.read_u32::<LittleEndian>().map(|e| e as u64))
            .collect::<Result<Vec<u64>, std::io::Error>>()?;

        grain_table.extend(
            l2_entries.iter()
                .enumerate()
                .filter(|(_, grain)| **grain != 0)
                .map(|(i, grain)| (start_cluster + i as u64 , *grain))
        );

        start_cluster += l2_len;
    }

    Ok(grain_table)
}

fn read_grain_table_sesparse<R>(
    h: &VmdkSeSparseMeta,
    src: &mut R
) -> Result<HashMap<u64, u64>, std::io::Error>
where
    R: Read + Seek
{
/*
    // read level 1
    src.seek(SeekFrom::Start(h.l1_offset))?;

    let l1_entries = (0..h.l1_len)
        .map(|_| src.read_u32::<LittleEndian>().map(|e| e as u64 * SECTOR_SIZE))
        .collect::<Result<Vec<u64>, std::io::Error>>()?;

    // read level 2
    let mut grain_table = HashMap::new();
    let mut start_cluster = 0;
    let total_clusters = h.sectors / h.cluster_sectors;

    for l2_offset in l1_entries {
        if start_cluster == total_clusters {
            // we've exhausted all the clusters; stop
            break;
        }

        let l2_len = h.l2_len.min(total_clusters - start_cluster);

        if l2_offset == 0 {
            // the data for this entry is in the parent
            start_cluster += l2_len;
            continue;
        }

        src.seek(SeekFrom::Start(l2_offset))?;

        let l2_entries = (0..l2_len)
            .map(|_| src.read_u32::<LittleEndian>().map(|e| e as u64))
            .collect::<Result<Vec<u64>, std::io::Error>>()?;

        grain_table.extend(
            l2_entries.iter()
                .enumerate()
                .filter(|(_, grain)| **grain != 0)
                .map(|(i, grain)| (start_cluster + i as u64 , *grain))
        );

        start_cluster += l2_len;
    }

    Ok(grain_table)

*/
    todo!()
}

fn read_extent<R, F>(
    ed: &ExtentDescription,
    filename: F,
    mut src: R
) -> Result<ExtentStorage, OpenError>
where
    R: Read + Seek + Clone + 'static,
    F: Into<String>
{
    let filename = filename.into();

    Ok(match &ed.kind {
        ExtentDescriptionInner::Sparse { .. } |
        ExtentDescriptionInner::VmfsSparse { .. } => {
            let header = read_header_sparse(src.clone())?;
            let grain_table = read_grain_table_sparse(&header, &mut src)?;

            ExtentStorage::Sparse(SparseStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                grain_table,
                grain_size: header.cluster_sectors,
                has_compressed_grain: header.compressed,
                zeroed_grain_table_entry: header.has_zero_grain
            })
        },
        ExtentDescriptionInner::SeSparse { .. } => {
            let header = read_header_sesparse(src.clone())?;
            let grain_table = read_grain_table_sesparse(&header, &mut src)?;

            ExtentStorage::Sparse(SparseStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                grain_table,
                grain_size: header.cluster_sectors,
                has_compressed_grain: true,
                zeroed_grain_table_entry: false
            })
        },
        ExtentDescriptionInner::Vmfs { .. } => {
            ExtentStorage::Flat(FlatStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                offset: 0
            })
        },
        ExtentDescriptionInner::Flat { offset, .. } => {
            ExtentStorage::Flat(FlatStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                offset: *offset
            })
        },
        _ => todo!("TODO: {:?} support", ed.kind)
    })
}

pub fn read_extents(
    image_url: &Url,
    eds: &[ExtentDescription],
    is_bin_and_singular: bool,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>,
    mut idx: usize
) -> Result<Vec<Extent>, OpenError>
{
    let mut extents = vec![];

    for ed in eds {
        let filename = ed.filename();

        let ed_url = image_url.join(filename)
            .map_err(|_| OpenErrorKind::BadPath(filename.into()))?;

        let src = source_for_url(&ed_url, &runtime)
            .or_else(|e|
                // if first filename is wrong and we are bin, try current file
                if is_bin_and_singular && &ed_url != image_url {
                    source_for_url(image_url, &runtime)
                }
                else {
                    Err(e)
                }
            )?;

        let seg_len = src.end();

        cache.lock().unwrap().add_source(idx, src);

        let crs = CacheReadSeek::new(
            cache.clone(),
            runtime.clone(),
            idx,
            seg_len
        );

        let storage = read_extent(ed, filename, crs)
            .map_err(|e| e.with_path(ed_url))?;

        extents.push(Extent {
            sectors: ed.sectors,
            start_sector: 0,
            storage
        });

        idx += 1;
    }

    for i in 1..extents.len() {
        extents[i].start_sector = extents[i - 1].start_sector + extents[i - 1].sectors;
    }

    Ok(extents)
}

#[cfg(test)]
mod test {
}
