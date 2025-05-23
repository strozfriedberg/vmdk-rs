use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
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

use crate::generated::vmware_cowd::*;
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
    pub total_size: u64,
    extents: LinkedList<Vec<ExtentDesc>>,
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

#[derive(Debug)]
struct VmdkSparseFileHeader {
    io: BytesReader,
    size_max: u64,
    size_grain: u64,
    grain_dir: u64,
    num_grain_table_entries: u32,
    zeroed_grain_table_entry: bool,
    has_compressed_grain: bool,
    descriptor: String,
}

impl VmdkReader {
    fn open_bin<T: AsRef<Path>>(f: T) -> Result<VmdkSparseFileHeader, SimpleError> {
        let io = BytesReader::open(f).unwrap();

        let first_bytes = io
            .read_bytes(4)
            .map_err(|e| SimpleError::new(format!("read_bytes error: {:?}", e)))?;

        io.seek(0)
            .map_err(|e| SimpleError::new(format!("seek error: {:?}", e)))?;

        if first_bytes == vec![0x43u8, 0x4Fu8, 0x57u8, 0x44u8]
        // COWD
        {
            match VmwareCowd::read_into::<_, VmwareCowd>(&io, None, None) {
                Ok(h) => {
                    return Ok(VmdkSparseFileHeader {
                        io,
                        size_max: *h.size_max() as u64,
                        size_grain: *h.size_grain() as u64,
                        grain_dir: *h.grain_dir() as u64,
                        num_grain_table_entries: *h.num_grain_table_entries(),
                        zeroed_grain_table_entry: false,
                        has_compressed_grain: false,
                        descriptor: "".to_string(),
                    });
                }
                Err(e) => {
                    return Err(SimpleError::new(format!(
                        "Error while deserializing VmwareCowd struct: {:?}",
                        e
                    )));
                }
            }
        } else if first_bytes == vec![0x4Bu8, 0x44u8, 0x4Du8, 0x56u8]
        // KDMV
        {
            let mut h = VmwareVmdk::read_into::<_, VmwareVmdk>(&io, None, None).map_err(|e| {
                SimpleError::new(format!(
                    "Error while deserializing VmwareVmdk struct: {:?}",
                    e
                ))
            })?;

            if *h.start_primary_grain() == -1
                && *h.compression_method() == VmwareVmdk_CompressionMethods::Deflate
            {
                // If the grain directory sector number value is -1 (0xffffffffffffffff) (GD_AT_END)
                // in a Stream-Optimized Compressed Sparse Extent there should be a secondary file header
                // stored at offset -1024 relative from the end of the file (stream)
                io.seek(io.size() - 1024)
                    .map_err(|e| SimpleError::new(format!("seek error: {:?}", e)))?;

                h = VmwareVmdk::read_into::<_, VmwareVmdk>(&io, None, None).map_err(|e| {
                    SimpleError::new(format!(
                        "Error while deserializing VmwareVmdk struct: {:?}",
                        e
                    ))
                })?;
            }

            return Ok(VmdkSparseFileHeader {
                io,
                size_max: *h.size_max() as u64,
                size_grain: *h.size_grain() as u64,
                grain_dir: if *h.flags().use_secondary_grain_dir() {
                    *h.start_secondary_grain() as u64
                } else {
                    *h.start_primary_grain() as u64
                },
                num_grain_table_entries: *h.num_grain_table_entries() as u32,
                zeroed_grain_table_entry: *h.flags().zeroed_grain_table_entry(),
                has_compressed_grain: *h.flags().has_compressed_grain(),
                descriptor: String::from_utf8_lossy(h.descriptor().unwrap().deref()).to_string(),
            });
        }

        Err(SimpleError::new(
            "No KDMV nor COWD headers detected".to_string(),
        ))
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
                            &captures[2], e
                        ))
                    })?;
                    let kind = Kind::from_str(&captures[3]).ok_or_else(|| SimpleError::new(format!(
                        "can't parse {} to Kind enum",
                        &captures[3]
                    )))?;
                    let filename = captures[4].to_string();
                    let offset = match captures.get(5) {
                        Some(v) => v.as_str().to_string().parse::<u64>().map_err(|e| {
                            SimpleError::new(format!(
                                "can't parse value '{}' to u64: {:?}",
                                &captures[5], e
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
            let (descriptor, is_bin) = Self::read_descriptor(current_fn.as_path())?;
            let extents0 = Self::read_extents(current_fn.as_path(), &descriptor, is_bin)?;
            let total_size0 = extents0.iter().fold(0u64, |acc, i| acc + i.sectors * 512);
            if total_size == 0 {
                total_size = total_size0;
            } else if total_size != total_size0 {
                return Err(SimpleError::new(format!(
                    "Size of all parent extent descriptors should equal to {}, we got {}, file {}",
                    total_size,
                    total_size0,
                    current_fn.to_string_lossy()
                )));
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

    fn read_descriptor<T: AsRef<Path>>(f: T) -> Result<(String, bool), SimpleError> {
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
            header?.descriptor
        };
        Ok((descriptor, !text_format))
    }

    fn read_extents<T: AsRef<Path>>(
        f: T,
        descriptor: &str,
        is_bin: bool,
    ) -> Result<Vec<ExtentDesc>, SimpleError> {
        let ed = Self::extract_ed_values(descriptor)?;
        let mut extents: Vec<ExtentDesc> = Vec::new();
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
            let mut ed_fn = f.as_ref().with_file_name(&i.filename);
            if is_bin && ed.len() == 1 && fs::metadata(&ed_fn).is_err() {
                // if 1st filename is wrong and we are bin - try to use current file
                ed_fn = f.as_ref().to_path_buf();
            }
            let mut has_compressed_grain = false;
            let mut zero_grain_table_entry = false;
            let grain_table = if i.kind == Kind::SPARSE || i.kind == Kind::VMFSSPARSE {
                let header = Self::open_bin(&ed_fn)?;
                has_compressed_grain = header.has_compressed_grain;
                zero_grain_table_entry = header.zeroed_grain_table_entry;
                grain_size = header.size_grain;
                Some(Self::read_grain_table(
                    &mut grain_table_start_index,
                    &header,
                    i.kind,
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

    fn read_grain_table(
        grain_table_start_index: &mut u64,
        h: &VmdkSparseFileHeader,
        kind: Kind,
    ) -> Result<HashMap<u64, u64>, SimpleError> {
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
            .map_err(|e| SimpleError::new(format!("seek err: {:?}", e)))?;
        let grain_dir_entries: Vec<u64> =
            h.io.read_bytes(number_of_grain_directory_entries as usize * 4)
                .map_err(|e| SimpleError::new(format!("read_bytes err: {:?}", e)))?
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
                } else {
                    h.num_grain_table_entries as usize
                }
            } else {
                4096
            };

            if *grain_table_offset == 0 {
                *grain_table_start_index += grain_table1_elems as u64;
                continue;
            }
            h.io.seek(*grain_table_offset as usize)
                .map_err(|e| SimpleError::new(format!("seek err: {:?}", e)))?;

            let grain_table: Vec<u64> =
                h.io.read_bytes(grain_table1_elems * 4)
                    .map_err(|e| SimpleError::new(format!("read_bytes err: {:?}", e)))?
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
            } else {
                *local_offset -= i.sectors * 512;
            }
        }

        None
    }

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
                SimpleError::new(format!(
                    "Sanity check failed for grain index {}",
                    grain_index
                )),
            ));
        }

        let mut buffer = vec![0u8; cgh.data_size as usize];
        file.read_exact(buffer.as_mut_slice())?;

        let mut decoder = DeflateDecoder::new(&*buffer.as_mut_slice());
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

                let sparse =
                    extent_desc.kind == Kind::SPARSE || extent_desc.kind == Kind::VMFSSPARSE;
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

                    match extent_desc.grain_table.as_ref().unwrap().get(&grain_index) {
                        None => {
                            // if this is last vmdk-file
                            if ex_pos == self.extents.len() - 1 {
                                remaining_buf[..remaining_grain_size].fill(0);
                            } else {
                                // check in next
                                continue;
                            }
                        }
                        Some(sector_num) => {
                            // handle zero GTE
                            if extent_desc.zero_grain_table_entry && *sector_num == 1 {
                                remaining_buf[..remaining_grain_size].fill(0);
                            } else {
                                let seek_pos = *sector_num * SECTOR_SIZE;
                                extent_desc
                                    .file
                                    .borrow_mut()
                                    .seek(SeekFrom::Start(seek_pos))?;
                                let grain_data = if extent_desc.has_compressed_grain {
                                    Self::read_and_decompress_grain(extent_desc, grain_index)?
                                } else {
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
                } else {
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
}
