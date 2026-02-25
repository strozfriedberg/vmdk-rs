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
    start_sector: u64,
    src: &mut R
) -> Result<HashMap<u64, u64>, std::io::Error>
where
    R: Read + Seek
{
    // read level 1
    src.seek(SeekFrom::Start(h.l1_offset))?;

    let l1_entries = (0..h.l1_len)
        .map(|_| src.read_u32::<LittleEndian>().map(|e| e as u64 * SECTOR_SIZE))
        .collect::<Result<Vec<u64>, std::io::Error>>()?;

    // read level 2
    let mut grain_table = HashMap::new();
// FIXME: should be start cluster of this extent, which won't be zero if there
// are multiple extents
    let mut cur_sector = start_sector;
    let end_sector = start_sector + h.sectors;

    for l2_offset in l1_entries {
        if cur_sector == end_sector {
            // we've exhausted all the sectors; stop
            break;
        }

        let l2_len = h.l2_len.min(h.sectors - (cur_sector - start_sector));

        if l2_offset == 0 {
            // the data for this entry is in the parent
            cur_sector += l2_len;
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
                .map(|(i, grain)| (cur_sector + i as u64 , *grain))
        );

        cur_sector += l2_len;
    }

    Ok(grain_table)
}

fn read_grain_table_sesparse<R>(
    h: &VmdkSeSparseMeta,
    start_sector: u64,
    src: &mut R
) -> Result<HashMap<u64, u64>, std::io::Error>
where
    R: Read + Seek
{
    /*
        SESPARSE extents differ from earlier sparse extent types:

            * table entries are 8 bytes instead of 4
            * l1 entries are rather baroque; see below for how they're read
            * l1 entries contain indices into the table of l2 tables, instead
              of offsets to l2 tables

        The only available reference implementation is QEMU's:

            https://github.com/qemu/qemu/blob/master/block/vmdk.c

        We've tried to document which values have which units.
    */

    // read level 1
    src.seek(SeekFrom::Start(h.l1_offset))?;

    let l1_entries = (0..h.l1_len)
        .map(|_| src.read_u64::<LittleEndian>())
        .collect::<Result<Vec<u64>, std::io::Error>>()?;

    // read level 2
    let mut grain_table = HashMap::new();
// FIXME: should be start cluster of this extent, which won't be zero if there
// are multiple extents
    let mut start_cluster = 0;
    let total_clusters = h.sectors / h.cluster_sectors;

    // size in bytes of an l2 table
    let l2_size = h.l2_len * 8;

    for l1_entry in l1_entries {
        if start_cluster == total_clusters {
            // we've exhausted all the clusters; stop
            break;
        }

        let l2_len = h.l2_len.min(total_clusters - start_cluster);

        // high nibble of l1 entries are 0 (unallocated) or 1 (allocated)

        if l1_entry == 0 {
            // Thank you Mario! But our princess is in another castle!
            // (the data for this entry is in the parent)
            start_cluster += l2_len;
            continue;
        }

        if l1_entry & 0xF000000000000000 != 0x1000000000000000 {
            return Err(std::io::Error::other("bad l1 entry"));
        }

        let l2_index = l1_entry & 0x0FFFFFFFFFFFFFFF;
        let l2_offset = h.l2_tables_offset + l2_index * l2_size;

        src.seek(SeekFrom::Start(l2_offset))?;

        let l2_entries = (0..l2_len)
            .map(|_| src.read_u64::<LittleEndian>())
            .collect::<Result<Vec<u64>, std::io::Error>>()?;

        for (i, &l2_entry) in l2_entries.iter().enumerate() {
            if l2_entry == 0 {
                // the data for this entry is in the parent
                continue;
            }

            let cluster_offset = match l2_entry & 0xF000000000000000 {
                0x1000000000000000 | 0x2000000000000000 => {
                    // zeroed grain
                    1
                },
                0x3000000000000000 => {
                    // allocted grain
                    h.clusters_offset + (
                        ((l2_entry & 0x0FFF000000000000) >> 48) |
                        ((l2_entry & 0x0000FFFFFFFFFFFF) << 12)
                    ) * h.cluster_sectors
                },
                _ => {
                    // 0 in high nibble means unallocated grain, which
                    // should not happen; anything else is also corrupt
                    return Err(std::io::Error::other("bad l2 entry"));
                }
            };

            grain_table.insert(start_cluster + i as u64, cluster_offset);
        }

        start_cluster += l2_len;
    }

    Ok(grain_table)
}

fn read_extent<R, F>(
    ed: &ExtentDescription,
    start_sector: u64,
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
            let grain_table = read_grain_table_sparse(
                &header,
                start_sector,
                &mut src
            )?;

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
            let grain_table = read_grain_table_sesparse(
                &header,
                start_sector,
                &mut src
            )?;

            ExtentStorage::Sparse(SparseStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                grain_table,
                grain_size: header.cluster_sectors,
                has_compressed_grain: false,
                zeroed_grain_table_entry: true
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

    let mut start_sector = 0;

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

        let storage = read_extent(ed, start_sector, filename, crs)
            .map_err(|e| e.with_path(ed_url))?;

        extents.push(Extent {
            sectors: ed.sectors,
            start_sector,
            storage
        });

        start_sector += ed.sectors;
        idx += 1;
    }

    Ok(extents)
}

#[cfg(test)]
mod test {
}
