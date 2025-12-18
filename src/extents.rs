use kaitai::ReadSeek;
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    sync::{Arc, Mutex}
};
use tokio::runtime::Runtime;
use url::Url;

use crate::{
    cache::Cache,
    cachereadseek::CacheReadSeek,
    errors::{DescriptorError, IoError, OpenError, OpenErrorKind},
    extent_description::{
        ExtentDescription,
        ExtentDescriptionInner,
        ExtentKind,
        extract_extent_descriptions
    },
    header::{VmdkSparseFileHeader, read_header},
    vmdk_reader::source_for_url,
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

fn read_grain_table(
    h: &mut VmdkSparseFileHeader,
    kind: ExtentKind
) -> Result<(HashMap<u64, u64>, u64), IoError> {
    let size_grain_bytes = h.size_grain * 512;
    let grain_table0_size = h.num_grain_table_entries as u64 * size_grain_bytes;
    let size_max = h.size_max * 512;
    let mut last_entry_special_size = false;
    let mut number_of_grain_directory_entries = h.num_grain_table_entries as u64;

    if kind == ExtentKind::Sparse {
        number_of_grain_directory_entries = size_max / grain_table0_size;
        if size_max % grain_table0_size > 0 {
            last_entry_special_size = true;
            number_of_grain_directory_entries += 1;
        }
    }

    let mut grain_table_all = HashMap::new();
    let mut grain_table_start_index = 0;

    // get and read metadata-0
    h.src.seek(SeekFrom::Start(h.grain_dir * 512))?;
//        .map_err(|e| IoError::SeekError(h.grain_dir as usize * 512, e))?;

    let mut buf = vec![0; number_of_grain_directory_entries as usize * 4];
    h.src.read_exact(&mut buf)?;

    let grain_dir_entries: Vec<u64> = buf.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64 * 512)
        .collect();

    // get and read metadata-1
    for (i, grain_table_offset) in grain_dir_entries.iter().enumerate() {
        let grain_table1_elems = if kind == ExtentKind::Sparse {
            if last_entry_special_size && i == grain_dir_entries.len() - 1 {
                let rest = size_max % grain_table0_size;
                rest.div_ceil(size_grain_bytes) as usize
            }
            else {
                h.num_grain_table_entries as usize
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
//            .map_err(|e| IoError::SeekError(*grain_table_offset as usize, e))?;
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

    Ok((grain_table_all, grain_table_start_index))
}

fn read_extent<T, F>(
    ed: &ExtentDescription,
    filename: F,
    src: T
) -> Result<ExtentStorage, OpenError>
where
    T: Read + Seek + Clone + 'static,
    F: Into<String>
{
    let filename = filename.into();

    Ok(match &ed.kind {
        ExtentDescriptionInner::Sparse { .. } |
        ExtentDescriptionInner::VmfsSparse { .. } => {
            let mut header = read_header(src.clone())?;
            let has_compressed_grain = header.has_compressed_grain;
            let zeroed_grain_table_entry = header.zeroed_grain_table_entry;
            let grain_size = header.size_grain;

            let (grain_table, grain_table_start_index) = read_grain_table(
                &mut header,
                (&ed.kind).into(),
            )?;

            ExtentStorage::Sparse(SparseStorage {
                file: Box::new(src) as Box<dyn ReadSeek>,
                filename,
                grain_table,
                grain_size,
                has_compressed_grain,
                zeroed_grain_table_entry
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
    descriptor: &str,
    header: Option<VmdkSparseFileHeader>,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>,
    mut idx: usize
) -> Result<Vec<Extent>, OpenError>
{
    let eds = extract_extent_descriptions(descriptor)
        .or(Err(DescriptorError::ParseExtentDescriptionError))?;

    let is_bin_and_singular = header.is_some() && eds.len() == 1;

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

        let storage = read_extent(&ed, filename, crs)
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
