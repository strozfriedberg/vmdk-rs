use regex::Regex;
use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    sync::LazyLock
};

use crate::errors::{DescriptorError, OpenError, OpenErrorKind};

const SECTOR_SIZE: u64 = 512;

pub fn read_descriptor_internal<R>(
   src: &mut R,
   offset: u64
) -> Result<String, std::io::Error>
where
    R: Read + Seek
{
    if offset > 0 {
        let mut buf = vec![];

        src.seek(SeekFrom::Start(offset * SECTOR_SIZE))?;

        let mut r = BufReader::new(src.take(20 * SECTOR_SIZE));
        let len = r.read_until(0, &mut buf)?;

        // read_until includes the delimiter
        Ok(String::from_utf8_lossy(&buf[..(len - 1)]).into())
    }
    else {
        Ok("".into())
    }
}

pub fn read_descriptor_file<R>(
    src: R
) -> Result<String, OpenError>
where
    R: Read
{
    // Read a line at a time until we know we have a descriptor file,
    // to avoid reading a giant file which is not a descriptor file
    // into memory.

    let mut r = BufReader::new(src);
    let mut desc = String::new();
    let mut line = String::new();

    loop {
        r.read_line(&mut line)?;
        desc += &line;

        match line.as_str().trim_end() {
            "# Disk DescriptorFile" => {
                // this is a descriptor file, read the rest
                r.read_to_string(&mut desc)?;
                return Ok(desc);
            },
            "" => line.clear(),
            _ => return Err(OpenError {
                path: "".into(),
                kind: OpenErrorKind::DescriptorError(
                    DescriptorError::UnrecognizedDescriptor
                )
            })
        }
    }
}

pub fn extract_parent_fn_hint(descriptor: &str) -> Option<String> {
    static PAT: LazyLock<Regex> = LazyLock::new(||
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_descriptor_file_ok() {
        let desc = r#"
# Disk DescriptorFile
version=1
encoding="UTF-8"
CID=8f67ca74
parentCID=0172e8a4
createType="vmfsSparse"
parentFileNameHint="vmfs_thick.vmdk"
# Extent description
RW 4096 VMFSSPARSE "vmfs_thick-000001-delta.vmdk"

# The Disk Data Base
#DDB

ddb.longContentID = "4b98b55ba6a6bc2e8fd6eb368f67ca74"
"#;

        assert_eq!(
            read_descriptor_file(desc.as_bytes()).unwrap(),
            desc
        );
    }

    #[test]
    fn test_read_descriptor_file_bad() {
        let desc = r#"


Bogus crap
"#;

        assert!(matches!(
            read_descriptor_file(desc.as_bytes()).unwrap_err(),
            OpenError {
                path: _,
                kind: OpenErrorKind::DescriptorError(
                    DescriptorError::UnrecognizedDescriptor
                )
            }
        ));
    }
}
