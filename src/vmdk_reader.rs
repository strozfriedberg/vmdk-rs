use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use kaitai::ReadSeek;
use regex::Regex;
use s3::{
    bucket::Bucket,
    creds::Credentials,
    region::Region
};
use std::{
    fmt::Debug,
    fs::{self, File},
    io::{self, BufReader, BufRead, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex}
};
use tokio::runtime::Runtime;
use tracing::debug;
use url::Url;

extern crate kaitai;

use crate::{
    bytessource::BytesSource,
    cache::Cache,
    cachereadseek::CacheReadSeek,
    dummycache::DummyCache,
    errors::{DescriptorError, InitError, OpenError, OpenErrorKind},
    filesource::FileSource,
    extents::{Extent, ExtentStorage, FlatStorage, SparseStorage, read_extents},
    header::{check_signature, read_header, VmdkSparseFileHeader},
    s3source::S3Source
};

const SECTOR_SIZE: u64 = 512;

pub struct VmdkReader {
    pub image_path: PathBuf,
    pub image_size: u64,
    extents: Vec<Vec<Extent>>,

    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>
}

impl Debug for VmdkReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmdkReader")
            .field("image_path", &self.image_path)
            .field("image_size", &self.image_size)
            .field("extents", &self.extents)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Requested offset {0} is beyond end of image {1}")]
    OffsetBeyondEnd(u64, u64),
    #[error("Offset {0} not found")]
    OffsetNotFound(u64),
    #[error("{0}")]
    IoError(#[from] io::Error)
}

fn read_descriptor_file<R>(
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

fn extract_parent_fn_hint(descriptor: &str) -> Option<String> {
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

fn extent_for_offset(
    extents: &mut [Extent],
    offset: u64
) -> Option<&mut Extent> {
    let sector = offset / SECTOR_SIZE;
    let i = extents.partition_point(|ex| ex.start_sector <= sector);

    match i {
        // offset before first extent
        0 => None,
        // offset is in extent i - 1
        i if sector < extents[i - 1].start_sector + extents[i - 1].sectors
            => Some(&mut extents[i - 1]),
        // offset is in a gap between extents i-1 and i
        _ => None
    }
}

// We're going off the rails on a crazy grain
#[derive(Debug, thiserror::Error)]
#[error("Sanity check failed for grain index {0}")]
struct CrazyGrainIndex(u64);

fn read_and_decompress_grain(
    file: &mut Box<dyn ReadSeek>,
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

fn path_or_url_to_url<P: AsRef<str>>(p: P) -> Option<Url> {
    match Url::parse(p.as_ref()) {
        // might be a path; make it absolute and reparse
        Err(url::ParseError::RelativeUrlWithoutBase) => Path::new(p.as_ref())
            .canonicalize()
            .map(Url::from_file_path)
            .map_err(|_| ())
// FIXME: use flatten after Rust 1.89
//            .flatten()
            .and_then(|r| r)
            .ok(),
        r => r.ok()
    }
}

pub fn source_for<P: AsRef<str>>(
    p: P,
    runtime: &Runtime
) -> Result<Box<dyn BytesSource + Send>, OpenError>
{
    let p = p.as_ref();

    let url = path_or_url_to_url(p)
        .ok_or(OpenErrorKind::BadPath(p.into()))?;

    match url.scheme() {
        "file" => {
            let len = std::fs::metadata(p)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(p))?
                .len();

            Ok(Box::new(FileSource { path: p.into(), len }))
        },
        "s3" => {
            let name = url.host_str()
                .ok_or(OpenErrorKind::BadPath(p.into()))?;

            let bucket = *Bucket::new(
                name,
                Region::UsEast1,
                Credentials::anonymous().unwrap()
            )
            .map_err(std::io::Error::other)
            .map_err(OpenError::from)
            .map_err(|e| e.with_path(p))?;

            let key = url.path();

            let (h, code) = runtime.block_on(bucket.head_object(key))
                .map_err(std::io::Error::other)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(p))?;

            assert_eq!(code, 200);
            let len = h.content_length.unwrap().try_into().unwrap();
            debug!("content-length: {len}");

            Ok(Box::new(S3Source::new(bucket, key.into(), len)))
        },
        _ => Err(OpenErrorKind::UnsupportedScheme(p.into()).into())
    }
}

fn handle_image<T: AsRef<Path>>(
    current_fn: T,
    idx: usize,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>
) -> Result<(Vec<Extent>, Option<String>), OpenError>
{
    // FIXME
    let current_fn_str = current_fn.as_ref().to_str()
        .ok_or_else(|| OpenErrorKind::BadPath("".into()))?;

    let src = source_for(current_fn_str, &runtime)?;
    let seg_len = src.end();

    cache.lock().unwrap().add_source(idx, src);

    let mut crs = CacheReadSeek::new(
        cache.clone(),
        runtime.clone(),
        idx,
        seg_len
    );

    let ft = check_signature(&mut crs)?;
    crs.seek(SeekFrom::Start(0))?;

    let (descriptor, header) = if ft.is_some() {
        let h = read_header(crs)?;
        let descriptor = h.descriptor.clone();
        (descriptor, Some(h))
    }
    else {
        (read_descriptor_file(crs)?, None)
    };

    let extents = read_extents(
        &current_fn,
        &descriptor,
        header,
        cache.clone(),
        runtime.clone(),
        idx
    )?;

    let next_fn = extract_parent_fn_hint(&descriptor);

    Ok((extents, next_fn))
}

impl VmdkReader {
    pub fn open<T: AsRef<Path>>(
        image_path: T
    ) -> Result<Self, OpenError>
    {
        let mut image_size = None;
        let mut extents = vec![];
        let mut current_fn = PathBuf::from(image_path.as_ref());

        let runtime = Arc::new(
            tokio::runtime::Runtime::new()
                .map_err(InitError::TokioRuntimeFailed)
                .map_err(OpenErrorKind::from)?
        );

        let c = DummyCache::new();
        let cache = Arc::new(Mutex::new(c));

        let mut idx = 0;

        let image_size = loop {
            let (extents0, next_fn) = handle_image(
                &current_fn,
                idx,
                cache.clone(),
                runtime.clone()
            )?;

            // size for all images must match
            let size0 = extents0.iter()
                .fold(0, |acc, i| acc + i.sectors) * SECTOR_SIZE;

            if image_size.is_none() {
                image_size = Some(size0);
            }
            else if let Some(s) = image_size && s != size0 {
                return Err(OpenError {
                    path: current_fn,
                    kind: OpenErrorKind::BadParentExtentDescriptorSize(
                        s, size0
                    )
                });
            }

            idx += extents0.len();
            extents.push(extents0);

            // keep going if we are not at the end of the image chain
            match next_fn {
                Some(next_fn) => current_fn.set_file_name(next_fn),
                None => break size0
            }
        };

        Ok(Self {
            image_path: image_path.as_ref().into(),
            image_size,
            extents,
            cache,
            runtime
        })
    }

    pub fn read_at_offset(
        &mut self,
        mut offset: u64,
        mut buf: &mut [u8]
    ) -> Result<usize, ReadError>
    {
        // don't start reading past the end
        let image_end = self.image_size;
        if offset > image_end {
            return Err(ReadError::OffsetBeyondEnd(offset, self.image_size));
        }

        // limit the buffer to the image end
        if offset + buf.len() as u64 > image_end {
            buf = &mut buf[..(image_end - offset) as usize];
        }

        let buf_beg = offset;
        let buf_end = offset + buf.len() as u64;

        let mut grain_size = 0;

        let ex_len = self.extents.len();

        while offset < buf_end {
            for (ex_pos, mut ex) in self.extents.iter_mut().enumerate() {
                let extent = extent_for_offset(&mut ex, offset)
                    .ok_or_else(|| ReadError::OffsetNotFound(offset))?;

                let (r, gs) = read_storage(
                    offset,
                    extent,
                    grain_size,
                    ex_pos == ex_len - 1,
                    &mut buf
                )?;

                grain_size = gs;

                match r {
                    None => continue,
                    Some(r) => {
                        offset += r as u64;
                        buf = &mut buf[r..];

                        // look for next block from the first extent descriptor
                        break;
                    }
                }
            }
        }

        Ok((offset - buf_beg) as usize)
    }
}

fn read_storage(
    offset: u64,
    extent: &mut Extent,
    mut grain_size: u64,
    is_last: bool,
    buf: &mut [u8]
) -> Result<(Option<usize>, u64), ReadError>
{
    // offset_in_extent is offset relative to the start of the extent
    let offset_in_extent = offset - extent.start_sector * SECTOR_SIZE;

    let buf_len = buf.len();
    let r;

    eprintln!("{grain_size}");

    match &mut extent.storage {
        ExtentStorage::Sparse(storage) => {
            grain_size = storage.grain_size * SECTOR_SIZE;

            r = if grain_size > 0 {
                buf_len.min((grain_size - (offset_in_extent % grain_size)) as usize)
            }
            else {
                buf_len
            };

            if !read_sparse(
                offset,
                is_last,
                storage,
                &mut buf[..r]
            )?
            {
                // not found, check in next file
                return Ok((None, grain_size));
            }
        },
        ExtentStorage::Flat(storage) => {
            // TODO: can this possibly be right? why does the
            // grain size matter for non-grained extents?
            r = if grain_size > 0 {
                buf_len.min((grain_size - (offset_in_extent % grain_size)) as usize)
            }
            else {
                buf_len
            };

            // FLAT, VMFS
            read_flat(
                offset_in_extent,
                storage,
                &mut buf[..r]
            )?;
        },
        ExtentStorage::Zero => todo!("ZERO support")
    }

    // look for next piece of data from the first extent descriptor
    Ok((Some(r), grain_size))
}

fn read_sparse(
    offset: u64,
    is_last: bool,
    storage: &mut SparseStorage,
    buf: &mut [u8]
) -> Result<bool, ReadError>
{
    // return value is whether we filled the buffer
    let grain_size = storage.grain_size * SECTOR_SIZE;
    let grain_index = offset / grain_size;

    match storage.grain_table.get(&grain_index) {
        None => {
            if is_last {
                // last vmdk file, zero-fill
                buf.fill(0);
                Ok(true)
            }
            else {
                // check in next
                Ok(false)
            }
        },
        Some(sector_num) => {
            if storage.zeroed_grain_table_entry && *sector_num == 1 {
                // handle zeroed GTE
                buf.fill(0);
            }
            else {
                let seek_pos = *sector_num * SECTOR_SIZE;
                storage.file.seek(SeekFrom::Start(seek_pos))?;

                // read whole grain
                let grain_data = if storage.has_compressed_grain {
                    read_and_decompress_grain(&mut storage.file, grain_index)?
                }
                else {
                    let mut data = vec![0u8; grain_size as usize];
                    storage.file.read_exact(&mut data)?;
                    data
                };

                let grain_data_offset = (offset % grain_size) as usize;

                buf.clone_from_slice(
                    &grain_data[grain_data_offset
                        ..grain_data_offset + buf.len()],
                );
            }
            Ok(true)
        }
    }
}

fn read_flat(
    local_offset: u64,
    storage: &mut FlatStorage,
    buf: &mut [u8]
) -> Result<(), ReadError>
{
    // FLAT, VMFS
    let f = &mut storage.file;
    // NB: only ExtentKind::Flat has nonzero offset
    f.seek(SeekFrom::Start(local_offset + storage.offset))?;
    f.read_exact(buf)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extent_for_offset() {
        let mut exts = [
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
            extent_for_offset(&mut exts, 0),
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
            extent_for_offset(&mut exts, 9 * SECTOR_SIZE),
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
            extent_for_offset(&mut exts, 10 * SECTOR_SIZE),
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
            extent_for_offset(&mut exts, 14 * SECTOR_SIZE),
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
            extent_for_offset(&mut exts, 15 * SECTOR_SIZE),
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
            extent_for_offset(&mut exts, 19 * SECTOR_SIZE),
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
            extent_for_offset(&mut exts, 20 * SECTOR_SIZE),
            None
        ));
    }

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
