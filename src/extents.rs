use kaitai::ReadSeek;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex}
};
use tokio::runtime::Runtime;

use crate::{
    cache::Cache,
    cachereadseek::CacheReadSeek,
    errors::{DescriptorError, IoError, OpenError},
    extent_description::{
        ExtentDescription,
        ExtentDescriptionInner,
        ExtentKind,
        extract_extent_descriptions
    },
    header::{VmdkSparseFileHeader, read_header},
    vmdk_reader::source_for,
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
                (rest / size_grain_bytes + if rest % size_grain_bytes > 0 { 1 } else { 0 })
                    as usize
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

fn read_extents_impl<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    header: Option<VmdkSparseFileHeader>,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>,
    mut idx: usize
) -> Result<Vec<Extent>, OpenError> {
    let eds = extract_extent_descriptions(descriptor)
        .or(Err(DescriptorError::ParseExtentDescriptionError))?;

    let is_bin_and_singular = header.is_some() && eds.len() == 1;

    let mut extents = vec![];

    for ed in eds {
        let filename = ed.filename();
// FIXME: probably wrong for S3?
        let mut ed_fn = image_path.as_ref().with_file_name(filename);
        if is_bin_and_singular && fs::metadata(&ed_fn).is_err() {
            // if first filename is wrong and we are bin, try current file
            ed_fn = image_path.as_ref().to_path_buf();
        }

// TODO: extract this from the loop?
        let filename = ed_fn.to_string_lossy().to_string();

        let src = source_for(&filename, &runtime)?;
        let seg_len = src.end(); 

        cache.lock().unwrap().add_source(idx, src);

        let crs = CacheReadSeek::new(
            cache.clone(),
            runtime.clone(),
            idx,
            seg_len
        );

        extents.push(Extent {
            sectors: ed.sectors,
            start_sector: 0,
            storage: read_extent(
                &ed,
                &filename,
                crs
            )?
        });

        idx += 1;
    }

    for i in 1..extents.len() {
        extents[i].start_sector = extents[i - 1].start_sector + extents[i - 1].sectors;
    }

    Ok(extents)
}

pub fn read_extents<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    header: Option<VmdkSparseFileHeader>,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>,
    idx: usize
) -> Result<Vec<Extent>, OpenError> {
    read_extents_impl(&image_path, descriptor, header, cache, runtime, idx)
        .map_err(|e| e.with_path(&image_path))
}

#[cfg(test)]
mod test {
}
