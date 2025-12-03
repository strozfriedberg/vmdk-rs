use std::str::FromStr;

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
pub enum ExtentDescriptionInner {
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
pub struct ExtentDescription {
    pub access_mode: AccessMode,
    pub sectors: u64,
    pub kind: ExtentDescriptionInner
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
// TODO: apparently 0 is possible here?
//                   offset: None,
                   offset: None | Some(0),
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
                _ => Err(ParseExtentDescriptionError)?
            }
        })
    }
}

pub fn extract_extent_descriptions(
    descriptor: &str
) -> Result<Vec<ExtentDescription>, ParseExtentDescriptionError>
{
    let mut eds = vec![];

    for line in descriptor.lines() {
        match line.trim_start().split_once(' ') {
            Some((a, _)) if a.parse::<AccessMode>().is_ok() => {
                eds.push(line.parse::<ExtentDescriptionLine>()?.try_into()?);
            },
            _ => continue
        }
    }

    Ok(eds)
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
