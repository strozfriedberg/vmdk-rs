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
    errors::{DescriptorError, OpenError, OpenErrorKind},
    extent_description::{ExtentDescription, ExtentDescriptionInner, ExtentKind},
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
    src: &mut R,
    kind: ExtentKind
) -> Result<HashMap<u64, u64>, std::io::Error>
where
    R: Read + Seek
{
    let size_grain_bytes = h.cluster_sectors * SECTOR_SIZE;
    let grain_table0_size = h.l1_size * size_grain_bytes;
    let size_max = h.sectors * SECTOR_SIZE;
    let mut last_entry_special_size = false;
    let mut number_of_grain_directory_entries = h.l1_size;

    if kind == ExtentKind::Sparse {
        number_of_grain_directory_entries = size_max / grain_table0_size;
        if !size_max.is_multiple_of(grain_table0_size) {
            last_entry_special_size = true;
            number_of_grain_directory_entries += 1;
        }
    }

    let mut grain_table_all = HashMap::new();
    let mut grain_table_start_index = 0;

    // get and read metadata-0
    src.seek(SeekFrom::Start(h.l1_table_offset))?;

    let mut buf = vec![0; number_of_grain_directory_entries as usize * 4];
    src.read_exact(&mut buf)?;

    let grain_dir_entries: Vec<u64> = buf.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64 * SECTOR_SIZE)
        .collect();

    // get and read metadata-1
    for (i, grain_table_offset) in grain_dir_entries.iter().enumerate() {
        let grain_table1_elems = if kind == ExtentKind::Sparse {
            if last_entry_special_size && i == grain_dir_entries.len() - 1 {
                let rest = size_max % grain_table0_size;
                rest.div_ceil(size_grain_bytes) as usize
            }
            else {
                h.l1_size as usize
            }
        }
        else {
            4096
        };

        if *grain_table_offset == 0 {
            grain_table_start_index += grain_table1_elems as u64;
            continue;
        }

        src.seek(SeekFrom::Start(*grain_table_offset))?;

        let mut buf = vec![0; grain_table1_elems * 4];
        src.read_exact(&mut buf)?;

        let grain_table: Vec<u64> = buf.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64)
            .collect();

        for (i, grain) in grain_table.iter().enumerate() {
            if *grain == 0 {
                continue;
            }
            let old = grain_table_all.insert(grain_table_start_index + i as u64, *grain);
            debug_assert!(old.is_none());
        }

        grain_table_start_index += grain_table.len() as u64;
    }

    Ok(grain_table_all)
}

fn read_grain_table_sesparse(
    h: &VmdkSeSparseMeta
) -> Result<HashMap<u64, u64>, std::io::Error> {
/*
    let size_grain_bytes = h.cluster_sectors * SECTOR_SIZE;
    let grain_table0_size = h.l1_size as u64 * size_grain_bytes;
    let size_max = h.sectors * SECTOR_SIZE;
    let mut last_entry_special_size = false;
    let mut number_of_grain_directory_entries = h.l1_size as u64;

    if kind == ExtentKind::Sparse {
        number_of_grain_directory_entries = size_max / grain_table0_size;
        if !size_max.is_multiple_of(grain_table0_size) {
            last_entry_special_size = true;
            number_of_grain_directory_entries += 1;
        }
    }

    let mut grain_table_all = HashMap::new();
    let mut grain_table_start_index = 0;

    // get and read metadata-0
    h.src.seek(SeekFrom::Start(h.l1_table_offset))?;

    let mut buf = vec![0; number_of_grain_directory_entries as usize * 4];
    h.src.read_exact(&mut buf)?;

    let grain_dir_entries: Vec<u64> = buf.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64 * SECTOR_SIZE)
        .collect();

    // get and read metadata-1
    for (i, grain_table_offset) in grain_dir_entries.iter().enumerate() {
        let grain_table1_elems = if kind == ExtentKind::Sparse {
            if last_entry_special_size && i == grain_dir_entries.len() - 1 {
                let rest = size_max % grain_table0_size;
                rest.div_ceil(size_grain_bytes) as usize
            }
            else {
                h.l1_size as usize
            }
        }
        else {
            4096
        };

        if *grain_table_offset == 0 {
            grain_table_start_index += grain_table1_elems as u64;
            continue;
        }

        h.src.seek(SeekFrom::Start(*grain_table_offset))?;

        let mut buf = vec![0; grain_table1_elems * 4];
        h.src.read_exact(&mut buf)?;

        let grain_table: Vec<u64> = buf.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64)
            .collect();

        for (i, grain) in grain_table.iter().enumerate() {
            if *grain == 0 {
                continue;
            }
            let old = grain_table_all.insert(grain_table_start_index + i as u64, *grain);
            debug_assert!(old.is_none());
        }

        grain_table_start_index += grain_table.len() as u64;
    }

    Ok(grain_table_all)
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
            let grain_table = read_grain_table_sparse(
                &header,
                &mut src,
                (&ed.kind).into()
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

            ExtentStorage::Sparse(SparseStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                grain_table: read_grain_table_sesparse(&header)?,
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
