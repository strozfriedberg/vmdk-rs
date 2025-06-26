use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf}
};

extern crate kaitai;
use self::kaitai::KStream;

use crate::errors::{DescriptorError, IoError, OpenError, OpenErrorKind};
use crate::header::{VmdkSparseFileHeader, open_header};

const SECTOR_SIZE: u64 = 512;

/*
RW 8323072 FLAT "CentOS 3-f001.vmdk" 0
RW 2162688 FLAT "CentOS 3-f002.vmdk" 0

sector_start = 0, sectors = 8323072
sector_start = 8323072, sectors = 2162688
 */
struct ExtentDesc {
    file: RefCell<File>,
    filename: String,
    start_sector: u64,
    sectors: u64,
    kind: Kind,
    // only if Kind == SPARSE
    grain_table: Option<HashMap<u64 /*sector*/, u64 /*real sector in file*/>>, // size size_grain * 512
    grain_size: u64,
    // only if Kind == FLAT
    offset: u64,
    has_compressed_grain: bool,
    zero_grain_table_entry: bool,
}

impl fmt::Debug for ExtentDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\n\tExtentDesc {{ sectors: {}, start_sector: {}, kind: {:?}, filename: {}, {} }}\n",
            self.sectors,
            self.start_sector,
            self.kind,
            self.filename,
            if self.grain_table.is_some() {
                format!(
                    "grain_table size {}",
                    self.grain_table.as_ref().unwrap().len()
                )
            } else {
                "flat".to_string()
            }
        )
    }
}

#[derive(Debug)]
pub struct VmdkReader {
    image_size: u64,
    extents: Vec<Vec<ExtentDesc>>,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Clone, Copy)]
enum Kind {
    SPARSE,
    FLAT,
    ZERO,
    VMFS,
    VMFSSPARSE,
    VMFSRDM,
    VMFSRAW,
}

impl Kind {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "SPARSE" => Some(Self::SPARSE),
            "FLAT" => Some(Self::FLAT),
            "ZERO" => Some(Self::ZERO),
            "VMFS" => Some(Self::VMFS),
            "VMFSSPARSE" => Some(Self::VMFSSPARSE),
            "VMFSRDM" => Some(Self::VMFSRDM),
            "VMFSRAW" => Some(Self::VMFSRAW),
            _ => panic!("Unknown extent descriptor KIND: {}", value),
        }
    }
}

#[derive(Debug)]
struct ED {
    sectors: u64,
    kind: Kind,
    filename: String,
    offset: u64, // value is specified only for flat extents and corresponds to the offset in the file
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

fn read_extents<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    is_bin: bool,
) -> Result<Vec<ExtentDesc>, OpenError> {
    let ed = extract_ed_values(descriptor)
        .map_err(OpenError::from)
        .map_err(|e| e.with_path(&image_path))?;

    let mut extents = vec![];
    let mut grain_size = 0;
    let mut grain_table_start_index = 0;

    for i in &ed {
        if i.kind != Kind::SPARSE
            && i.kind != Kind::FLAT
            && i.kind != Kind::VMFS
            && i.kind != Kind::VMFSSPARSE
        {
            todo!("TODO: support {:?}", i.kind);
        }

        let mut ed_fn = image_path.as_ref().with_file_name(&i.filename);
        if is_bin && ed.len() == 1 && fs::metadata(&ed_fn).is_err() {
            // if 1st filename is wrong and we are bin - try to use current file
            ed_fn = image_path.as_ref().to_path_buf();
        }

        let mut has_compressed_grain = false;
        let mut zero_grain_table_entry = false;
        let grain_table = match i.kind {
            Kind::SPARSE | Kind::VMFSSPARSE => {
                let header = open_header(&ed_fn)?;
                has_compressed_grain = header.has_compressed_grain;
                zero_grain_table_entry = header.zeroed_grain_table_entry;
                grain_size = header.size_grain;
                Some(
                    read_grain_table(
                        &mut grain_table_start_index,
                        &header,
                        i.kind,
                    )?
                )
            },
            _ => None
        };

        let file = File::open(&ed_fn)
            .map_err(OpenError::from)
            .map_err(|e| e.with_path(&image_path))?;

        let ed = ExtentDesc {
            file: RefCell::new(file),
            filename: ed_fn.to_string_lossy().to_string(),
            start_sector: 0, // will be updated later (see below)
            sectors: i.sectors,
            kind: i.kind,
            grain_table,
            grain_size,
            offset: i.offset,
            has_compressed_grain,
            zero_grain_table_entry,
        };
        if ed.kind != Kind::SPARSE && ed.kind != Kind::VMFSSPARSE {
            // skip this check (file on disk could be bigger)
            debug_assert!(std::fs::metadata(&ed_fn).unwrap().len() <= ed.sectors * 512);
        }
        extents.push(ed);
    }

    for i in 1..extents.len() {
        extents[i].start_sector = extents[i - 1].start_sector + extents[i - 1].sectors;
    }

    Ok(extents)
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

fn extract_ed_values(descriptor: &str) -> Result<Vec<ED>, DescriptorError> {
    static PAT: Lazy<Regex> = Lazy::new(||
        Regex::new(r#"^(\w+)\s+(\d+)\s+(\w+)\s+"([^"]+)"(?:\s+(\d+)(?:\s+.+)?)?$"#)
            .expect("bad regex")
    );

    let mut ed = vec![];

    for line in descriptor.lines() {
        if line.starts_with("RW") ||
           line.starts_with("RDONLY") ||
           line.starts_with("NOACCESS")
        {
            if let Some(captures) = PAT.captures(line) {
                // ignore access mode (captures[1])
                let sectors = captures[2].parse::<u64>()
                    .map_err(|_| DescriptorError::U64ParseError(captures[2].into()))?;

                let kind = Kind::from_str(&captures[3]).ok_or_else(||
                    DescriptorError::KindParseError(captures[3].into()))?;

                let filename = captures[4].to_string();

                let offset = match captures.get(5) {
                    Some(v) => v.as_str().parse::<u64>()
                        .map_err(|_| DescriptorError::U64ParseError(v.as_str().into()))?,
                    None => 0,
                };

                ed.push(ED {
                    sectors,
                    kind,
                    filename,
                    offset,
                });
            }
        }
    }

    Ok(ed)
}

fn read_grain_table(
    grain_table_start_index: &mut u64,
    h: &VmdkSparseFileHeader,
    kind: Kind,
) -> Result<HashMap<u64, u64>, IoError> {
    let size_grain_bytes = h.size_grain * 512;
    let grain_table0_size = h.num_grain_table_entries as u64 * size_grain_bytes;
    let size_max = h.size_max * 512;
    let mut last_entry_special_size = false;
    let mut number_of_grain_directory_entries = h.num_grain_table_entries as u64;

    if kind == Kind::SPARSE {
        number_of_grain_directory_entries = size_max / grain_table0_size;
        if size_max % grain_table0_size > 0 {
            last_entry_special_size = true;
            number_of_grain_directory_entries += 1;
        }
    }

    let mut grain_table_all: HashMap<u64, u64> = HashMap::new();

    // get and read metadata-0
    h.io.seek(h.grain_dir as usize * 512)
        .map_err(|e| IoError::SeekError(h.grain_dir as usize * 512, e))?;

    let grain_dir_entries: Vec<u64> =
        h.io.read_bytes(number_of_grain_directory_entries as usize * 4)
            .map_err(IoError::ReadError)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64 * 512)
            .collect();

    // get and read metadata-1
    for (i, grain_table_offset) in grain_dir_entries.iter().enumerate() {
        let grain_table1_elems = if kind == Kind::SPARSE {
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
            *grain_table_start_index += grain_table1_elems as u64;
            continue;
        }

        h.io.seek(*grain_table_offset as usize)
            .map_err(|e| IoError::SeekError(*grain_table_offset as usize, e))?;

        let grain_table: Vec<u64> =
            h.io.read_bytes(grain_table1_elems * 4)
                .map_err(IoError::ReadError)?
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64)
                .collect();

        for (i, grain) in grain_table.iter().enumerate() {
            if *grain == 0 {
                continue;
            }
            let old = grain_table_all.insert(*grain_table_start_index + i as u64, *grain);
            debug_assert!(old.is_none());
        }

        *grain_table_start_index += grain_table.len() as u64;
    }

    Ok(grain_table_all)
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
                current_fn.as_path(),
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
