use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    fs::{self, File},
    io::{self, BufReader, BufRead, Read, Seek, SeekFrom},
    path::{Path, PathBuf}
};

extern crate kaitai;

use crate::errors::{DescriptorError, OpenError, OpenErrorKind};
use crate::extents::{Extent, ExtentStorage, read_extents};
use crate::header::open_header;

const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
pub struct VmdkReader {
    pub image_path: PathBuf,
    pub image_size: u64,
    extents: Vec<Vec<Extent>>
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Requested offset {0} is beyond end of image {1}")]
    OffsetBeyondEnd(u64, u64),
    #[error("{0}")]
    IoError(#[from] io::Error)
}

fn read_descriptor<T: AsRef<Path>>(
    image_path: T
) -> Result<(String, bool), OpenError>
{
// FIXME: don't swallow errors from open_bin
    match open_header(&image_path) {
        Ok(header) => Ok((header.descriptor, true)),
        Err(_) => {
            // maybe this is a raw descriptor file
            let f = File::open(&image_path)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(&image_path))?;

            let f = BufReader::new(f);

            for line in f.lines() {
                let line = line
                    .map_err(OpenError::from)
                    .map_err(|e| e.with_path(&image_path))?;

                match line.as_str() {
                    "# Disk DescriptorFile" => break,
                    "" => continue,
                    _ => return Err(OpenError {
                        path: image_path.as_ref().into(),
                        kind: OpenErrorKind::DescriptorError(
                            DescriptorError::UnrecognizedDescriptor
                        )
                    })
                }
            }

            Ok((
                fs::read_to_string(&image_path)
                    .map_err(OpenError::from)
                    .map_err(|e| e.with_path(&image_path))?,
                false
            ))

        }
    }
}

fn extract_parent_fn_hint(descriptor: &str) -> Option<String> {
    static PAT: Lazy<Regex> = Lazy::new(||
        Regex::new(r#"^parentFileNameHint="([^"]+)"#)
            .expect("bad regex")
    );

    for line in descriptor.lines() {
        if let Some(captures) = PAT.captures(line) {
            return Some(captures[1].to_string());
        }
    }
    None
}

fn extent_for_offset(
    extents: &[Extent],
    offset: u64
) -> Option<&Extent> {
    let sector = offset / 512;
    let i = extents.partition_point(|ex| ex.start_sector <= sector);

    match i {
        // offset before first extent
        0 => None,
        // offset is in extent i-1
        i if sector < extents[i-1].start_sector + extents[i-1].sectors => Some(&extents[i-1]),
        // offset is in a gap between extents i-1 and i
        _ => None
    }
}

// We're going off the rails on a crazy grain
#[derive(Debug, thiserror::Error)]
#[error("Sanity check failed for grain index {0}")]
struct CrazyGrainIndex(u64);

fn read_and_decompress_grain(
    file: &mut File,
    grain_index: u64,
) -> std::io::Result<Vec<u8>> {

    #[derive(Debug)]
    struct CompressedGrainHeader {
        _lba: u64,
        data_size: u32,
    }

    let cgh = CompressedGrainHeader {
        _lba: file.read_u64::<LittleEndian>()?,
        data_size: file.read_u32::<LittleEndian>()?,
    };

    let header: u16 = file.read_u16::<BigEndian>()?;

    // sanity check against expected zlib stream header values...
    if header % 31 != 0 || header & 0x0F00 != 8 << 8 || header & 0x0020 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            CrazyGrainIndex(grain_index)
        ));
    }

    let mut buffer = vec![0u8; cgh.data_size as usize];
    file.read_exact(buffer.as_mut_slice())?;

    let mut decoder = DeflateDecoder::new(&*buffer.as_mut_slice());
    let mut decoded_data = vec![];
    decoder.read_to_end(&mut decoded_data)?;

    Ok(decoded_data)
}

impl VmdkReader {
    pub fn open<T: AsRef<Path>>(
        image_path: T
    ) -> Result<Self, OpenError>
    {
        let mut total_size = 0;
        let mut extents = vec![];
        let mut current_fn = PathBuf::from(image_path.as_ref());

        loop {
            let (descriptor, is_bin) = read_descriptor(&current_fn)?;
            let extents0 = read_extents(&current_fn, &descriptor, is_bin)?;

            let total_size0 = extents0.iter().fold(0, |acc, i| acc + i.sectors) * 512;
            if total_size == 0 {
                total_size = total_size0;
            }
            else if total_size != total_size0 {
                return Err(OpenError {
                    path: current_fn,
                    kind: OpenErrorKind::BadParentExtentDescriptorSize(
                        total_size, total_size0
                    )
                });
            }

            extents.push(extents0);

            if let Some(next_fn) = extract_parent_fn_hint(&descriptor) {
                current_fn.set_file_name(next_fn);
            }
            else {
                break;
            }
        }

        Ok(Self {
            image_path: image_path.as_ref().into(),
            image_size: total_size,
            extents,
        })
    }

    pub fn read_at_offset(
        &self,
        mut offset: u64,
        buf: &mut [u8]
    ) -> Result<usize, ReadError>
    {
        if offset > self.image_size {
            return Err(ReadError::OffsetBeyondEnd(offset, self.image_size));
        }

        let mut bytes_read = 0;
        let mut grain_size = 0;
        let mut eof = false;

        while bytes_read < buf.len() && !eof {
            for (ex_pos, ex) in self.extents.iter().enumerate() {
                let Some(extent) = extent_for_offset(ex, offset) else {
                    eof = true;
                    break;
                };

                let local_offset = offset - extent.start_sector * 512;

                let remaining_buf = &mut buf[bytes_read..];
                let remaining_size = remaining_buf.len();
                let remaining_grain_size;

                match &extent.storage {
                    ExtentStorage::Sparse(storage) => {
                        grain_size = storage.grain_size * SECTOR_SIZE;

                        remaining_grain_size = if grain_size > 0 {
                            remaining_size.min((grain_size - (local_offset % grain_size)) as usize)
                        }
                        else {
                            remaining_size
                        };

                        // calculate grain index and offset
                        let grain_index = offset / grain_size;
                        let grain_data_offset = (offset % grain_size) as usize;

                        match storage.grain_table.get(&grain_index) {
                            None => {
                                // if this is last vmdk-file
                                if ex_pos == self.extents.len() - 1 {
                                    remaining_buf[..remaining_grain_size].fill(0);
                                }
                                else {
                                    // check in next
                                    continue;
                                }
                            },
                            Some(sector_num) => {
                                // handle zero GTE
                                if storage.zeroed_grain_table_entry && *sector_num == 1 {
                                    remaining_buf[..remaining_grain_size].fill(0);
                                }
                                else {
                                    let seek_pos = *sector_num * SECTOR_SIZE;
                                    storage.file
                                        .borrow_mut()
                                        .seek(SeekFrom::Start(seek_pos))?;
                                    let grain_data = if storage.has_compressed_grain {
                                        read_and_decompress_grain(&mut storage.file.borrow_mut(), grain_index)?
                                    }
                                    else {
                                        // calculate real sector and read whole grain
                                        let mut data = vec![0u8; grain_size as usize];
                                        storage.file.borrow_mut().read_exact(&mut data)?;
                                        data
                                    };
                                    remaining_buf[..remaining_grain_size].clone_from_slice(
                                        &grain_data[grain_data_offset
                                            ..grain_data_offset + remaining_grain_size],
                                    );
                                }
                            }
                        }
                    },
                    ExtentStorage::Flat(storage) => {
                        remaining_grain_size = if grain_size > 0 {
                            remaining_size.min((grain_size - (local_offset % grain_size)) as usize)
                        }
                        else {
                            remaining_size
                        };

                        // FLAT, VMFS

                        let mut f = storage.file.borrow_mut();

                        // NB: only ExtentKind::Flat has nonzero offset
                        f.seek(SeekFrom::Start(local_offset + storage.offset))?;
                        f.read_exact(&mut remaining_buf[..remaining_grain_size])?;
                    },
                    ExtentStorage::Zero => todo!("ZERO support")
                }

                bytes_read += remaining_grain_size;
                offset += remaining_grain_size as u64;
                // look for next piece of data from the first extent descriptor
                break;
            }
        }

        Ok(bytes_read)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extent_for_offset() {
        let exts = [
            Extent {
                start_sector: 0,
                sectors: 10,
                storage: ExtentStorage::Zero
            },
            Extent {
                start_sector: 10,
                sectors: 5,
                storage: ExtentStorage::Zero
            },
            Extent {
                start_sector: 15,
                sectors: 5,
                storage: ExtentStorage::Zero
            }
        ];

        // start of 0
        assert!(matches!(
            extent_for_offset(&exts, 0),
            Some(
                Extent {
                    start_sector: 0,
                    sectors: 10,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // end of 0
        assert!(matches!(
            extent_for_offset(&exts, 9 * 512),
            Some(
                Extent {
                    start_sector: 0,
                    sectors: 10,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // start of 1
        assert!(matches!(
            extent_for_offset(&exts, 10 * 512),
            Some(
                Extent {
                    start_sector: 10,
                    sectors: 5,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // end of 1
        assert!(matches!(
            extent_for_offset(&exts, 14 * 512),
            Some(
                Extent {
                    start_sector: 10,
                    sectors: 5,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // start of 2
        assert!(matches!(
            extent_for_offset(&exts, 15 * 512),
            Some(
                Extent {
                    start_sector: 15,
                    sectors: 5,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // end of 2
        assert!(matches!(
            extent_for_offset(&exts, 19 * 512),
            Some(
                Extent {
                    start_sector: 15,
                    sectors: 5,
                    storage: ExtentStorage::Zero
                }
            )
        ));

        // past the end
        assert!(matches!(
            extent_for_offset(&exts, 20 * 512),
            None
        ));
    }
}
