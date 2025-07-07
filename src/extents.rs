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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AccessMode {
    NoAccess,
    RdOnly,
    Rw
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
// TODO
#[error("")]
pub struct ParseAccessModeError;

impl FromStr for AccessMode {
    type Err = ParseAccessModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NOACCESS" => Ok(Self::NoAccess),
            "RDONLY" => Ok(Self::RdOnly),
            "RW" => Ok(Self::Rw),
            _ => Err(ParseAccessModeError)
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExtentKind {
    Sparse,
    Flat,
    Zero,
    Vmfs,
    VmfsSparse,
    VmfsRdm,
    VmfsRaw,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
// TODO
#[error("")]
pub struct ParseExtentKindError;

impl FromStr for ExtentKind {
    type Err = ParseExtentKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SPARSE" => Ok(Self::Sparse),
            "FLAT" => Ok(Self::Flat),
            "ZERO" => Ok(Self::Zero),
            "VMFS" => Ok(Self::Vmfs),
            "VMFSSPARSE" => Ok(Self::VmfsSparse),
            "VMFSRDM" => Ok(Self::VmfsRdm),
            "VMFSRAW" => Ok(Self::VmfsRaw),
            _ => Err(ParseExtentKindError)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ExtentDescriptionLine {
    access_mode: AccessMode,
    sectors: u64,
    kind: ExtentKind,
    filename: Option<String>,
    offset: Option<u64>
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
// TODO
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
        let kind = tok.parse::<ExtentKind>()
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

#[derive(Debug, PartialEq, Eq, Clone)]
enum ExtentDescriptionInner {
    Sparse {
        filename: String
    },
    Flat {
        filename: String,
        offset: u64
    },
    Zero,
    Vmfs {
        filename: String
    },
    VmfsSparse {
        filename: String
    },
    VmfsRdm {
        filename: String
    },
    VmfsRaw {
        filename: String
    }
}

impl From<&ExtentDescriptionInner> for ExtentKind {
    fn from(edi: &ExtentDescriptionInner) -> Self {
        match edi {
            ExtentDescriptionInner::Sparse { .. } => ExtentKind::Sparse,
            ExtentDescriptionInner::Flat { .. } => ExtentKind::Flat,
            ExtentDescriptionInner::Zero => ExtentKind::Zero,
            ExtentDescriptionInner::Vmfs { .. } => ExtentKind::Vmfs,
            ExtentDescriptionInner::VmfsSparse { .. } => ExtentKind::VmfsSparse,
            ExtentDescriptionInner::VmfsRdm { .. } => ExtentKind::VmfsRdm,
            ExtentDescriptionInner::VmfsRaw { .. } => ExtentKind::VmfsRaw
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
        Ok(ExtentDescription {
            access_mode: edl.access_mode,
            sectors: edl.sectors,
            kind: match edl {
                ExtentDescriptionLine {
                    kind: ExtentKind::Zero,
                    filename: None,
                    offset: None,
                    ..
                } => ExtentDescriptionInner::Zero,
                ExtentDescriptionLine {
                    kind: ExtentKind::Flat,
                    filename: Some(filename),
                    offset: Some(offset),
                    ..
                } => ExtentDescriptionInner::Flat { filename, offset },
                ExtentDescriptionLine {
                    kind: ExtentKind::Sparse,
                    filename: Some(filename),
                    offset: None,
                    ..
                } => ExtentDescriptionInner::Sparse { filename },
                ExtentDescriptionLine {
                    kind: ExtentKind::Vmfs,
                    filename: Some(filename),
                    offset: None,
                    ..
                } => ExtentDescriptionInner::Vmfs { filename },
                ExtentDescriptionLine {
                    kind: ExtentKind::VmfsSparse,
                    filename: Some(filename),
                    offset: None,
                    ..
                } => ExtentDescriptionInner::VmfsSparse { filename },
                ExtentDescriptionLine {
                    kind: ExtentKind::VmfsRdm,
                    filename: Some(filename),
                    offset: None,
                    ..
                } => ExtentDescriptionInner::VmfsRdm { filename },
                ExtentDescriptionLine {
                    kind: ExtentKind::VmfsRaw,
                    filename: Some(filename),
                    offset: None,
                    ..
                } => ExtentDescriptionInner::VmfsRaw { filename },
                _ => return Err(ParseExtentDescriptionError)
            }
        })
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

// TODO: rename to Extent, split into variants

pub struct Extent {

}

pub struct ExtentDesc {
    pub file: RefCell<File>,
    pub filename: String,
    pub start_sector: u64,
    pub sectors: u64,
    pub kind: ExtentKind,
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
    kind: ExtentKind,
) -> Result<HashMap<u64, u64>, IoError> {
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

fn read_extent<T: AsRef<Path>>(
    ed: &ExtentDescription,
    image_path: T,
    is_bin_and_singular: bool
) -> Result<ExtentDesc, OpenError>
{
    let filename = match &ed.kind {
        ExtentDescriptionInner::Sparse { filename } |
        ExtentDescriptionInner::Flat { filename, .. } |
        ExtentDescriptionInner::Vmfs { filename } |
        ExtentDescriptionInner::VmfsSparse { filename } => filename,
        _ => todo!("TODO: {:?} support", ed.kind)
    };

    let mut ed_fn = image_path.as_ref().with_file_name(filename);
    if is_bin_and_singular && fs::metadata(&ed_fn).is_err() {
        // if 1st filename is wrong and we are bin - try to use current file
        ed_fn = image_path.as_ref().to_path_buf();
    }

    let mut grain_size = 0;
    let mut grain_table_start_index = 0;
    let mut has_compressed_grain = false;
    let mut zeroed_grain_table_entry = false;
    let grain_table = match ed.kind {
        ExtentDescriptionInner::Sparse { .. } |
        ExtentDescriptionInner::VmfsSparse { .. } => {
            let header = open_header(&ed_fn)?;
            has_compressed_grain = header.has_compressed_grain;
            zeroed_grain_table_entry = header.zeroed_grain_table_entry;
            grain_size = header.size_grain;
            Some(
                read_grain_table(
                    &mut grain_table_start_index,
                    &header,
                    (&ed.kind).into(),
                )?
            )
        },
        _ => None
    };

    let file = File::open(&ed_fn)?;

    let offset = match ed.kind {
        ExtentDescriptionInner::Flat { offset, .. } => Some(offset),
        _ => None
    };

    let ex = ExtentDesc {
        file: RefCell::new(file),
        filename: ed_fn.to_string_lossy().to_string(),
        start_sector: 0, // will be updated later (see below)
        sectors: ed.sectors,
        kind: (&ed.kind).into(),
        grain_table,
        grain_size,
        offset,
        has_compressed_grain,
        zeroed_grain_table_entry,
    };

    if ex.kind != ExtentKind::Sparse && ex.kind != ExtentKind::VmfsSparse {
        // skip this check (file on disk could be bigger)
        debug_assert!(std::fs::metadata(&ed_fn).unwrap().len() <= ex.sectors * 512);
    }

    Ok(ex)
}

fn read_extents_impl<T: AsRef<Path>>(
    image_path: T,
    descriptor: &str,
    is_bin: bool
) -> Result<Vec<ExtentDesc>, OpenError> {
    let eds = extract_extent_descriptions(descriptor)
        .or(Err(DescriptorError::ParseExtentDescriptionError))?;

    let is_bin_and_singular = is_bin && eds.len() == 1;

    let mut extents = vec![];

    for i in &eds {
        extents.push(read_extent(i, &image_path, is_bin_and_singular)?);
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
                access_mode: AccessMode::Rw,
                sectors: 4192256,
                kind: ExtentKind::Sparse,
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
                access_mode: AccessMode::Rw,
                sectors: 1048576,
                kind: ExtentKind::Flat,
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
                kind: ExtentKind::ZERO,
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
