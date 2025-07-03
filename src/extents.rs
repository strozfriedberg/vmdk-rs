use kaitai::KStream;
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
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AccessMode {
    NOACCESS,
    RDONLY,
    RW
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("")]
pub struct ParseAccessModeError;

impl FromStr for AccessMode {
    type Err = ParseAccessModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NOACCESS" => Ok(Self::NOACCESS),
            "RDONLY" => Ok(Self::RDONLY),
            "RW" => Ok(Self::RW),
            _ => Err(ParseAccessModeError)
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    SPARSE,
    FLAT,
    ZERO,
    VMFS,
    VMFSSPARSE,
    VMFSRDM,
    VMFSRAW,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("")]
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


#[derive(Debug, PartialEq, Eq)]
struct ExtentDescriptionLine {
    access_mode: AccessMode,
    sectors: u64,
    kind: Kind,
    filename: Option<String>,
    offset: Option<u64>
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("")]
pub struct ParseExtentDescriptionError;

impl FromStr for ExtentDescriptionLine {
    type Err = ParseExtentDescriptionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // read the access mode
        let (tok, s) = s.trim_start().split_once(' ')
            .ok_or(ParseExtentDescriptionError)?;
        let access_mode = tok.parse::<AccessMode>()
            .or(Err(ParseExtentDescriptionError))?;

        // read the sector count
        let (tok, s) = s.trim_start().split_once(' ')
            .ok_or(ParseExtentDescriptionError)?;
        let sectors = tok.parse::<u64>()
            .or(Err(ParseExtentDescriptionError))?;

        // read the extent kind
        let (tok, s) = s.trim_start().split_once(' ')
            .ok_or(ParseExtentDescriptionError)?;
        let kind = tok.parse::<Kind>()
            .or(Err(ParseExtentDescriptionError))?;

        // read the optional filename and offset
        let s = s.trim_start();
        let (filename, offset) = if s.is_empty() {
            (None, None)
        }
        else {
            // read the filename
            let (tok, s) = s.strip_prefix('"')
                .ok_or(ParseExtentDescriptionError)?
                .rsplit_once('"')
                .ok_or(ParseExtentDescriptionError)?;
            let filename = Some(tok.to_string());

            // read the offset
            let s = s.trim_start();
            let offset = match s.is_empty() {
                true => None,
                false => Some(s.parse::<u64>()
                    .or(Err(ParseExtentDescriptionError))?)
            };

            (filename, offset)
        };

        Ok(
            ExtentDescriptionLine {
                access_mode,
                sectors,
                kind,
                filename,
                offset
            }
        )
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Eq, Clone)]
enum ExtentDescriptionInner {
    SPARSE {
        filename: String
    },
    FLAT {
        filename: String,
        offset: u64
    },
    ZERO,
    VMFS {
        filename: String
    },
    VMFSSPARSE {
        filename: String
    },
    VMFSRDM {
        filename: String
    },
    VMFSRAW {
        filename: String
    }
}

impl From<&ExtentDescriptionInner> for Kind {
    fn from(edi: &ExtentDescriptionInner) -> Self {
        match edi {
            ExtentDescriptionInner::SPARSE { .. } => Kind::SPARSE,
            ExtentDescriptionInner::FLAT { .. } => Kind::FLAT,
            ExtentDescriptionInner::ZERO => Kind::ZERO,
            ExtentDescriptionInner::VMFS { .. } => Kind::VMFS,
            ExtentDescriptionInner::VMFSSPARSE { .. } => Kind::VMFSSPARSE,
            ExtentDescriptionInner::VMFSRDM { .. } => Kind::VMFSRDM,
            ExtentDescriptionInner::VMFSRAW { .. } => Kind::VMFSRAW
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ExtentDescription {
    access_mode: AccessMode,
    sectors: u64,
    kind: ExtentDescriptionInner
}

impl TryFrom<ExtentDescriptionLine> for ExtentDescription {
    type Error = ParseExtentDescriptionError;

    fn try_from(edl: ExtentDescriptionLine) -> Result<Self, Self::Error> {
        match edl {
            ExtentDescriptionLine {
                kind: Kind::ZERO,
                filename: None,
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::ZERO
            }),
            ExtentDescriptionLine {
                kind: Kind::FLAT,
                filename: Some(filename),
                offset: Some(offset),
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::FLAT {
                    filename,
                    offset
                }
            }),
            ExtentDescriptionLine {
                kind: Kind::SPARSE,
                filename: Some(filename),
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::SPARSE {
                    filename
                }
            }),
            ExtentDescriptionLine {
                kind: Kind::VMFS,
                filename: Some(filename),
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::VMFS {
                    filename
                }
            }),
            ExtentDescriptionLine {
                kind: Kind::VMFSSPARSE,
                filename: Some(filename),
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::VMFSSPARSE {
                    filename
                }
            }),
            ExtentDescriptionLine {
                kind: Kind::VMFSRDM,
                filename: Some(filename),
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::VMFSRDM {
                    filename
                }
            }),
            ExtentDescriptionLine {
                kind: Kind::VMFSRAW,
                filename: Some(filename),
                offset: None,
                ..
            } => Ok(ExtentDescription {
                access_mode: edl.access_mode,
                sectors: edl.sectors,
                kind: ExtentDescriptionInner::VMFSRAW {
                    filename
                }
            }),
            _ => Err(ParseExtentDescriptionError)
        }
    }
}

fn extract_extent_descriptions(
    descriptor: &str
) -> Result<Vec<ExtentDescription>, ParseExtentDescriptionError>
{
    let mut eds = vec![];

    for line in descriptor.lines() {
        match line.trim_start().split_once(' ') {
            Some((a, _)) if a.parse::<AccessMode>().is_ok() => {
                eds.push(line.parse::<ExtentDescriptionLine>()?.try_into()?);
            },
            _ => continue,
        }
    }

    Ok(eds)
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
    pub offset: Option<u64>,
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
    let eds = extract_extent_descriptions(descriptor)
        .or(Err(DescriptorError::ParseExtentDescriptionError))?;

    let mut extents = vec![];
    let mut grain_size = 0;
    let mut grain_table_start_index = 0;

    for i in &eds {
        let filename = match &i.kind {
            ExtentDescriptionInner::SPARSE { filename } |
            ExtentDescriptionInner::FLAT { filename, .. } |
            ExtentDescriptionInner::VMFS { filename } |
            ExtentDescriptionInner::VMFSSPARSE { filename } => filename,
            _ => todo!("TODO: {:?} support", i.kind)
        };

        let mut ed_fn = image_path.as_ref().with_file_name(filename);
        if is_bin && eds.len() == 1 && fs::metadata(&ed_fn).is_err() {
            // if 1st filename is wrong and we are bin - try to use current file
            ed_fn = image_path.as_ref().to_path_buf();
        }

        let mut has_compressed_grain = false;
        let mut zeroed_grain_table_entry = false;
        let grain_table = match i.kind {
            ExtentDescriptionInner::SPARSE { .. } |
            ExtentDescriptionInner::VMFSSPARSE { .. } => {
                let header = open_header(&ed_fn)?;
                has_compressed_grain = header.has_compressed_grain;
                zeroed_grain_table_entry = header.zeroed_grain_table_entry;
                grain_size = header.size_grain;
                Some(
                    read_grain_table(
                        &mut grain_table_start_index,
                        &header,
                        (&i.kind).into(),
                    )?
                )
            },
            _ => None
        };

        let file = File::open(&ed_fn)?;

        let offset = match i.kind {
            ExtentDescriptionInner::FLAT { offset, .. } => Some(offset),
            _ => None
        };

        let ed = ExtentDesc {
            file: RefCell::new(file),
            filename: ed_fn.to_string_lossy().to_string(),
            start_sector: 0, // will be updated later (see below)
            sectors: i.sectors,
            kind: (&i.kind).into(),
            grain_table,
            grain_size,
            offset,
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn read_extent_description_line_sparse() {
        let ed = r#"RW 4192256 SPARSE "test-f001.vmdk""#;
        assert_eq!(
            ed.parse::<ExtentDescriptionLine>().unwrap(),
            ExtentDescriptionLine {
                access_mode: AccessMode::RW,
                sectors: 4192256,
                kind: Kind::SPARSE,
                filename: Some("test-f001.vmdk".into()),
                offset: None
            }
        );


    }

    #[test]
    fn read_extent_description_line_flat() {
        let ed = r#"RW 1048576 FLAT "test-f001.vmdk" 0"#;
        assert_eq!(
            ed.parse::<ExtentDescriptionLine>().unwrap(),
            ExtentDescriptionLine {
                access_mode: AccessMode::RW,
                sectors: 1048576,
                kind: Kind::FLAT,
                filename: Some("test-f001.vmdk".into()),
                offset: Some(0)
            }
        );
    }

/*
    #[test]
    fn read_extent_description_line_zero() {
        let ed = r#"RW 12345 ZERO"#;
        assert_eq!(
            ed.parse::<ExtentDescriptionLine>().unwrap(),
            ExtentDescriptionLine {
                sectors: 12345,
                kind: Kind::ZERO,
                filename: "test-f001.vmdk",
                offset: Some(0)
            }
        );
    }
*/

/*
TODO: extent description tests for:
    ZERO,
    VMFS,
    VMFSSPARSE,
    VMFSRDM,
    VMFSRAW,

TODO: What happens if the filename has a double quote in it?
TODO: What happens if the filename has a space in it?
TODO: extent description test for filename containing a space
TODO: extent description test for filename containing a double quote
TODO: can extent description filenames be single-quote delimited?
*/
}
