use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use libflate::deflate::Decoder;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::LinkedList;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;

extern crate kaitai;
use self::kaitai::*;

use crate::generated::vmware_vmdk::*;

use simple_error::SimpleError;

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
}

impl fmt::Debug for ExtentDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\n\tExtentDesc {{ sectors: {}, start_sector: {}, kind: {:?}, filename: {}, grain_table size {} sectors }}",
            self.sectors,
            self.start_sector,
            self.kind,
            self.filename,
            if self.grain_table.is_some() {
                self.grain_table.as_ref().unwrap().len()
            } else {
                0
            }
        )
    }
}

#[derive(Debug)]
pub struct VmdkReader {
    pub total_size: u64,
    extents: LinkedList<Vec<ExtentDesc>>,
}

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

impl VmdkReader {
    fn open_bin<T: AsRef<Path>>(f: T) -> Result<OptRc<VmwareVmdk>, SimpleError> {
        let _io = BytesReader::open(f).unwrap();
        let res: KResult<OptRc<VmwareVmdk>> = VmwareVmdk::read_into(&_io, None, None);
        let header: OptRc<VmwareVmdk>;

        match res {
            Ok(_) => {
                header = res.unwrap();
            }
            Err(e) => {
                return Err(SimpleError::new(format!(
                    "Error while deserializing VmwareVmdk struct: {:?}",
                    e
                )));
            }
        }
        Ok(header)
    }

    fn extract_parent_fn_hint(descriptor: &str) -> Option<String> {
        for line in descriptor.lines() {
            if let Some(captures) = regex::Regex::new(r#"^parentFileNameHint="([^"]+)"#)
                .unwrap()
                .captures(line)
            {
                return Some(captures[1].to_string());
            }
        }
        None
    }

    fn extract_ed_values(descriptor: &str) -> Result<Vec<ED>, SimpleError> {
        let mut ed: Vec<ED> = Vec::new();

        for line in descriptor.lines() {
            if line.starts_with("RW") || line.starts_with("RDONLY") || line.starts_with("NOACCESS")
            {
                if let Some(captures) = regex::Regex::new(
                    r#"^(\w+)\s+(\d+)\s+(\w+)\s+"([^"]+)"(?:\s+(\d+)(?:\s+.+)?)?$"#,
                )
                .unwrap()
                .captures(line)
                {
                    // ignore access mode (captures[1])
                    let sectors = captures[2].to_string().parse::<u64>().map_err(|e| {
                        SimpleError::new(format!(
                            "can't parse value '{}' to u64: {:?}",
                            captures[2].to_string(),
                            e
                        ))
                    })?;
                    let kind = Kind::from_str(&captures[3].to_string()).ok_or(SimpleError::new(
                        format!("can't parse {} to Kind enum", captures[3].to_string()),
                    ))?;
                    let filename = captures[4].to_string();
                    let offset = match captures.get(5) {
                        Some(v) => v.as_str().to_string().parse::<u64>().map_err(|e| {
                            SimpleError::new(format!(
                                "can't parse value '{}' to u64: {:?}",
                                captures[5].to_string(),
                                e
                            ))
                        })?,
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

    pub fn open<T: AsRef<Path>>(f: T) -> Result<Self, SimpleError> {
        let mut total_size = 0;
        let mut extents: LinkedList<Vec<ExtentDesc>> = LinkedList::new();
        let mut current_fn = PathBuf::from(f.as_ref());
        loop {
            let descriptor = Self::read_descriptor(current_fn.as_path())?;
            let extents0 = Self::read_extents(current_fn.as_path(), &descriptor)?;
            let total_size0 = extents0.iter().fold(0u64, |acc, i| acc + i.sectors * 512);
            if total_size == 0 {
                total_size = total_size0;
            } else {
                if total_size != total_size0 {
                    return Err(SimpleError::new(format!(
                        "Size of all parent extent descriptors should equal to {}, we got {}, file {}",
                        total_size, total_size0, current_fn.to_string_lossy()
                    )));
                }
            }
            extents.push_back(extents0);
            if let Some(next_fn) = Self::extract_parent_fn_hint(&descriptor) {
                current_fn.set_file_name(next_fn);
            } else {
                break;
            }
        }
        Ok(Self {
            total_size,
            extents,
        })
    }

    fn read_descriptor<T: AsRef<Path>>(f: T) -> Result<String, SimpleError> {
        let header = Self::open_bin(&f);
        let text_format = header.is_err();
        let descriptor = if text_format {
            fs::read_to_string(&f).map_err(|e| {
                SimpleError::new(format!(
                    "Error while reading the file {}: {:?}",
                    f.as_ref().to_string_lossy(),
                    e
                ))
            })?
        } else {
            String::from_utf8(header?.descriptor().unwrap().deref().to_vec()).unwrap()
        };
        Ok(descriptor)
    }

    fn read_extents<T: AsRef<Path>>(
        f: T,
        descriptor: &str,
    ) -> Result<Vec<ExtentDesc>, SimpleError> {
        let mut ed = Self::extract_ed_values(descriptor)?;
        let mut extents: Vec<ExtentDesc> = Vec::new();
        let mut grain_size = 0;
        let mut grain_table_start_index = 0;
        for i in &mut ed {
            if i.kind != Kind::SPARSE && i.kind != Kind::FLAT && i.kind != Kind::VMFS {
                todo!("TODO: support {:?}", i.kind);
            }
            let ed_fn = f.as_ref().with_file_name(&i.filename);
            let mut has_compressed_grain = false;
            let grain_table = if i.kind == Kind::SPARSE {
                let header = Self::open_bin(&ed_fn)?;
                has_compressed_grain = *header.flags().has_compressed_grain();
                grain_size = *header.size_grain() as u64;
                Some(Self::read_grain_table(
                    &mut grain_table_start_index,
                    header,
                )?)
            } else {
                None
            };
            let file = File::open(&ed_fn).map_err(|e| {
                SimpleError::new(format!(
                    "Can't open file {}, error: {e:?}",
                    f.as_ref().to_string_lossy()
                ))
            })?;
            let ed = ExtentDesc {
                file: RefCell::new(file),
                filename: i.filename.clone(),
                start_sector: 0, // will be updated later (see below)
                sectors: i.sectors,
                kind: i.kind,
                grain_table,
                grain_size,
                offset: i.offset,
                has_compressed_grain,
            };
            assert!(std::fs::metadata(&ed_fn).unwrap().len() <= ed.sectors * 512);
            extents.push(ed);
        }
        for i in 1..extents.len() {
            extents[i].start_sector = extents[i - 1].start_sector + extents[i - 1].sectors;
        }
        Ok(extents)
    }

    fn read_grain_table(
        grain_table_start_index: &mut u64,
        h: OptRc<VmwareVmdk>,
    ) -> Result<HashMap<u64, u64>, SimpleError> {
        let grain_table0_size = *h.num_grain_table_entries() as i64 * (*h.size_grain() * 512);
        let size_max = *h.size_max() * 512;
        let mut number_of_grain_directory_entries = size_max / grain_table0_size;
        if size_max % grain_table0_size > 0 {
            // TODO handle len of last entry
            number_of_grain_directory_entries += 1;
        }
        let mut grain_table_all: HashMap<u64, u64> = HashMap::new();
        let grain_dir = if *h.flags().use_secondary_grain_dir() {
            h.grain_secondary()
        } else {
            h.grain_primary()
        };
        let sparse_value = if *h.flags().zeroed_grain_table_entry() {
            1
        } else {
            0
        };
        // get and read metadata-0
        if let Ok(grains) = &grain_dir {
            let truncated_grains = &(*grains)[..number_of_grain_directory_entries as usize * 4];
            let grain_dir_entries: Vec<u64> = truncated_grains
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64 * 512)
                .collect();
            // get and read metadata-1
            let grain_table1_size = *h.num_grain_table_entries() as usize * 4;
            for grain_table_offset in grain_dir_entries {
                if grain_table_offset == sparse_value {
                    continue;
                }
                h._io()
                    .seek(grain_table_offset as usize)
                    .map_err(|e| SimpleError::new(format!("seek err: {:?}", e)))?;

                let grain_table: Vec<u64> = h
                    ._io()
                    .read_bytes(grain_table1_size as usize)
                    .map_err(|e| SimpleError::new(format!("read_bytes err: {:?}", e)))?
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as u64)
                    .collect();

                for i in 0..grain_table.len() {
                    if grain_table[i] == sparse_value {
                        continue;
                    }
                    let old =
                        grain_table_all.insert(*grain_table_start_index + i as u64, grain_table[i]);
                    debug_assert!(old.is_none());
                }
                *grain_table_start_index += grain_table.len() as u64;
            }
        }
        Ok(grain_table_all)
    }

    fn get_extent_from_offset<'a>(
        extents: &'a Vec<ExtentDesc>,
        offset: u64,
        local_offset: &mut u64,
    ) -> Option<&'a ExtentDesc> {
        let sector_num = offset / 512;

        for i in 0..extents.len() {
            if sector_num >= extents[i].start_sector
                && sector_num < extents[i].start_sector + extents[i].sectors
            {
                return Some(&extents[i]);
            } else {
                *local_offset -= extents[i].sectors * 512;
            }
        }

        None
    }

    fn read_and_decompress_grain(
        extent_desc: &ExtentDesc,
        grain_index: u64,
    ) -> std::io::Result<Vec<u8>> {
        let sector_num = *extent_desc
            .grain_table
            .as_ref()
            .unwrap()
            .get(&grain_index)
            .unwrap();
        let seek_pos = sector_num * SECTOR_SIZE;
        extent_desc
            .file
            .borrow_mut()
            .seek(SeekFrom::Start(seek_pos))?;

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

        //sanity check against expected zlib stream header values...
        if header % 31 != 0 || header & 0x0F00 != 8 << 8 || header & 0x0020 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                SimpleError::new(format!(
                    "Sanity check failed for grain index {}",
                    grain_index
                )),
            ));
        }

        let mut buffer = vec![0u8; cgh.data_size as usize];
        file.read_exact(buffer.as_mut_slice())?;

        let mut decoder = Decoder::new(&*buffer.as_mut_slice());
        let mut decoded_data = Vec::new();
        decoder.read_to_end(&mut decoded_data)?;

        Ok(decoded_data)
    }

    pub fn read_at_offset(&self, mut offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut bytes_read = 0;
        let mut grain_size = 0;
        let mut eof = false;

        while bytes_read < buf.len() && !eof {
            for (ex_pos, ex) in self.extents.iter().enumerate() {
                let mut local_offset = offset;
                let extent_desc = match Self::get_extent_from_offset(ex, offset, &mut local_offset)
                {
                    Some(e) => e,
                    None => {
                        eof = true;
                        break;
                    }
                };

                let sparse = extent_desc.kind == Kind::SPARSE;
                if sparse {
                    grain_size = extent_desc.grain_size * SECTOR_SIZE;
                }

                let remaining_buf = &mut buf[bytes_read..];
                let remaining_size = remaining_buf.len();
                let remaining_grain_size = if grain_size > 0 {
                    remaining_size.min((grain_size - (local_offset % grain_size)) as usize)
                } else {
                    remaining_size
                };

                if sparse {
                    // calculate grain index and offset
                    let grain_index = offset / grain_size;
                    let grain_data_offset = (offset % grain_size) as usize;

                    if !extent_desc
                        .grain_table
                        .as_ref()
                        .unwrap()
                        .contains_key(&grain_index)
                    {
                        // if this is last vmdk-file
                        if ex_pos == self.extents.len() - 1 {
                            remaining_buf[..remaining_grain_size].fill(0);
                        } else {
                            // check in next
                            continue;
                        }
                    } else {
                        let grain_data = if extent_desc.has_compressed_grain {
                            Self::read_and_decompress_grain(extent_desc, grain_index)?
                        } else {
                            // calculate real sector and read whole grain
                            let mut data = vec![0u8; grain_size as usize];

                            let sector_num = *extent_desc
                                .grain_table
                                .as_ref()
                                .unwrap()
                                .get(&grain_index)
                                .unwrap();
                            let seek_pos = sector_num * SECTOR_SIZE;
                            extent_desc
                                .file
                                .borrow_mut()
                                .seek(SeekFrom::Start(seek_pos))?;
                            extent_desc.file.borrow_mut().read_exact(&mut data)?;

                            data
                        };
                        remaining_buf[..remaining_grain_size].clone_from_slice(
                            &grain_data
                                [grain_data_offset..grain_data_offset + remaining_grain_size],
                        );
                    }
                } else {
                    // FLAT, VMFS

                    // handle offset only if Kind::FLAT
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
}
