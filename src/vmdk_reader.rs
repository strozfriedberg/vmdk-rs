use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf}
};

extern crate kaitai;

use crate::errors::{OpenError, OpenErrorKind};
use crate::extents::{ExtentDesc, Kind, read_extents};
use crate::header::open_header;

const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
pub struct VmdkReader {
    image_size: u64,
    extents: Vec<Vec<ExtentDesc>>,
}

fn read_descriptor<T: AsRef<Path>>(
    image_path: T
) -> Result<(String, bool), OpenError>
{
// FIXME: don't swallow errors from open_bin
    match open_header(&image_path) {
        Ok(header) => Ok((header.descriptor, true)),
        Err(_) => Ok((
            fs::read_to_string(&image_path)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(&image_path))?,
            false
        ))
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

fn get_extent_from_offset<'a>(
    extents: &'a Vec<ExtentDesc>,
    offset: u64,
    local_offset: &mut u64,
) -> Option<&'a ExtentDesc> {
    let sector_num = offset / 512;

    for i in extents {
        if sector_num >= i.start_sector && sector_num < i.start_sector + i.sectors {
            return Some(i);
        }
        else {
            *local_offset -= i.sectors * 512;
        }
    }

    None
}

#[derive(Debug, thiserror::Error)]
#[error("Sanity check failed for grain index {0}")]
struct CrazyGrainIndex(u64);

fn read_and_decompress_grain(
    extent_desc: &ExtentDesc,
    grain_index: u64,
) -> std::io::Result<Vec<u8>> {
    #[derive(Debug)]
    struct CompressedGrainHeader {
        _lba: u64,
        data_size: u32,
    }

    let mut file = extent_desc.file.borrow_mut();

    let cgh = CompressedGrainHeader {
        _lba: file.read_u64::<LittleEndian>().unwrap(),
        data_size: file.read_u32::<LittleEndian>().unwrap(),
    };

    let header: u16 = file.read_u16::<BigEndian>().unwrap();

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
            let extents0 = read_extents(
                &current_fn,
                &descriptor,
                is_bin
            )?;

            let total_size0 = extents0.iter().fold(0u64, |acc, i| acc + i.sectors * 512);
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
            image_size: total_size,
            extents,
        })
    }

    pub fn read_at_offset(&self, mut offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut bytes_read = 0;
        let mut grain_size = 0;
        let mut eof = false;

        while bytes_read < buf.len() && !eof {
            for (ex_pos, ex) in self.extents.iter().enumerate() {
                let mut local_offset = offset;
                let extent_desc = match get_extent_from_offset(ex, offset, &mut local_offset)
                {
                    Some(e) => e,
                    None => {
                        eof = true;
                        break;
                    }
                };

                let sparse =
                    extent_desc.kind == Kind::SPARSE || extent_desc.kind == Kind::VMFSSPARSE;
                if sparse {
                    grain_size = extent_desc.grain_size * SECTOR_SIZE;
                }

                let remaining_buf = &mut buf[bytes_read..];
                let remaining_size = remaining_buf.len();
                let remaining_grain_size = if grain_size > 0 {
                    remaining_size.min((grain_size - (local_offset % grain_size)) as usize)
                }
                else {
                    remaining_size
                };

                if sparse {
                    // calculate grain index and offset
                    let grain_index = offset / grain_size;
                    let grain_data_offset = (offset % grain_size) as usize;

                    match extent_desc.grain_table.as_ref().unwrap().get(&grain_index) {
                        None => {
                            // if this is last vmdk-file
                            if ex_pos == self.extents.len() - 1 {
                                remaining_buf[..remaining_grain_size].fill(0);
                            }
                            else {
                                // check in next
                                continue;
                            }
                        }
                        Some(sector_num) => {
                            // handle zero GTE
                            if extent_desc.zero_grain_table_entry && *sector_num == 1 {
                                remaining_buf[..remaining_grain_size].fill(0);
                            }
                            else {
                                let seek_pos = *sector_num * SECTOR_SIZE;
                                extent_desc
                                    .file
                                    .borrow_mut()
                                    .seek(SeekFrom::Start(seek_pos))?;
                                let grain_data = if extent_desc.has_compressed_grain {
                                    read_and_decompress_grain(extent_desc, grain_index)?
                                }
                                else {
                                    // calculate real sector and read whole grain
                                    let mut data = vec![0u8; grain_size as usize];
                                    extent_desc.file.borrow_mut().read_exact(&mut data)?;
                                    data
                                };
                                remaining_buf[..remaining_grain_size].clone_from_slice(
                                    &grain_data[grain_data_offset
                                        ..grain_data_offset + remaining_grain_size],
                                );
                            }
                        }
                    }
                }
                else {
                    // FLAT, VMFS

                    // handle extent offset only if Kind::FLAT
                    if extent_desc.kind == Kind::FLAT && extent_desc.offset > 0 {
                        local_offset += extent_desc.offset;
                    }

                    extent_desc
                        .file
                        .borrow_mut()
                        .seek(SeekFrom::Start(local_offset))?;

                    extent_desc
                        .file
                        .borrow_mut()
                        .read_exact(&mut remaining_buf[..remaining_grain_size])?;
                }
                bytes_read += remaining_grain_size;
                offset += remaining_grain_size as u64;
                // look for next piece of data from the first extent descriptor
                break;
            }
        }

        Ok(bytes_read)
    }

    pub fn total_size(&self) -> u64 {
        self.image_size
    }
}
