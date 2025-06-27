use kaitai::KStream;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    fs::{self, File},
    path::Path,
    str::FromStr
};

use crate::errors::{DescriptorError, IoError, OpenError};
use crate::header::{VmdkSparseFileHeader, open_header};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    SPARSE,
    FLAT,
    ZERO,
    VMFS,
    VMFSSPARSE,
    VMFSRDM,
    VMFSRAW,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseKindError;

impl FromStr for Kind {
    type Err = ParseKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SPARSE" => Ok(Self::SPARSE),
            "FLAT" => Ok(Self::FLAT),
            "ZERO" => Ok(Self::ZERO),
            "VMFS" => Ok(Self::VMFS),
            "VMFSSPARSE" => Ok(Self::VMFSSPARSE),
            "VMFSRDM" => Ok(Self::VMFSRDM),
            "VMFSRAW" => Ok(Self::VMFSRAW),
            _ => Err(ParseKindError)
        }
    }
}

/*
RW 8323072 FLAT "CentOS 3-f001.vmdk" 0
RW 2162688 FLAT "CentOS 3-f002.vmdk" 0

sector_start = 0, sectors = 8323072
sector_start = 8323072, sectors = 2162688
 */
pub struct ExtentDesc {
    pub file: RefCell<File>,
    pub filename: String,
    pub start_sector: u64,
    pub sectors: u64,
    pub kind: Kind,
    // only if Kind == SPARSE
    pub grain_table: Option<HashMap<u64 /*sector*/, u64 /*real sector in file*/>>, // size size_grain * 512
    pub grain_size: u64,
    // only if Kind == FLAT
    pub offset: u64,
    pub has_compressed_grain: bool,
    pub zeroed_grain_table_entry: bool
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
            self.grain_table.as_ref().map_or(
                "flat".into(),
                |gt| format!("grain_table size {}", gt.len())
            )
        )
    }
}

#[derive(Debug)]
struct ED {
    sectors: u64,
    kind: Kind,
    filename: String,
    offset: Option<u64> // value is specified only for flat extents and corresponds to the offset in the file
}

fn extract_ed_values(descriptor: &str) -> Result<Vec<ED>, DescriptorError> {
    static PAT: Lazy<Regex> = Lazy::new(||
        Regex::new(r#"^(\w+)\s+(\d+)\s+(\w+)\s+"([^"]+)"(?:\s+(\d+)(?:\s+.+)?)?$"#)
            .expect("bad regex")
    );

    let mut ed = vec![];

    for captures in descriptor
        .lines()
        .filter(|line|
            line.starts_with("RW") ||
            line.starts_with("RDONLY") ||
            line.starts_with("NOACCESS")
        )
        .filter_map(|line| PAT.captures(line))
    {
        // ignore access mode (captures[1])
        let sectors = captures[2].parse::<u64>()
            .map_err(|_| DescriptorError::U64ParseError(captures[2].into()))?;

        let kind = captures[3].parse::<Kind>()
            .map_err(|_| DescriptorError::KindParseError(captures[3].into()))?;

        let filename = captures[4].to_string();

        let offset = match captures.get(5) {
            Some(v) => Some(v.as_str().parse::<u64>()
                .map_err(|_| DescriptorError::U64ParseError(v.as_str().into()))?),
            None => None
        };

        ed.push(ED { sectors, kind, filename, offset });
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

    let mut grain_table_all = HashMap::new();

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

pub fn read_extents_impl<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    is_bin: bool
) -> Result<Vec<ExtentDesc>, OpenError> {
    let ed = extract_ed_values(descriptor)?;

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
        let mut zeroed_grain_table_entry = false;
        let grain_table = match i.kind {
            Kind::SPARSE | Kind::VMFSSPARSE => {
                let header = open_header(&ed_fn)?;
                has_compressed_grain = header.has_compressed_grain;
                zeroed_grain_table_entry = header.zeroed_grain_table_entry;
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

        let file = File::open(&ed_fn)?;

        let ed = ExtentDesc {
            file: RefCell::new(file),
            filename: ed_fn.to_string_lossy().to_string(),
            start_sector: 0, // will be updated later (see below)
            sectors: i.sectors,
            kind: i.kind,
            grain_table,
            grain_size,
            offset: i.offset.unwrap_or(0),
            has_compressed_grain,
            zeroed_grain_table_entry,
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

pub fn read_extents<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    is_bin: bool
) -> Result<Vec<ExtentDesc>, OpenError> {
    read_extents_impl(&image_path, descriptor, is_bin)
        .map_err(|e| e.with_path(&image_path))
}
