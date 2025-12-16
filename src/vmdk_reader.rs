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
    collections::BTreeMap,
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
    s3source::S3Source,
    spans::{insert_span, remove_span}
};

const SECTOR_SIZE: u64 = 512;

pub struct VmdkReader {
    pub image_path: PathBuf,
    pub image_size: u64,

    spans: Vec<(u64, (u64, usize))>,
    extents: Vec<Extent>,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>
}

impl Debug for VmdkReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmdkReader")
            .field("image_path", &self.image_path)
            .field("image_size", &self.image_size)
            .field("spans", &self.spans)
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
        let mut current_fn = PathBuf::from(image_path.as_ref());

        let runtime = Arc::new(
            tokio::runtime::Runtime::new()
                .map_err(InitError::TokioRuntimeFailed)
                .map_err(OpenErrorKind::from)?
        );

        let c = DummyCache::new();
        let cache = Arc::new(Mutex::new(c));

        let mut idx = 0;
        let mut spans: BTreeMap<u64, (u64, usize)> = BTreeMap::new();
        let mut uncovered: BTreeMap<u64, u64> = BTreeMap::new();
        let mut extents = vec![];

        let image_size = 'img_loop: loop {
            let (img_extents, parent_fn) = handle_image(
                &current_fn,
                idx,
                cache.clone(),
                runtime.clone()
            )?;

            // size for all images must match
            let size = img_extents.iter()
                .fold(0, |acc, i| acc + i.sectors) * SECTOR_SIZE;

            if image_size.is_none() {
                image_size = Some(size);
                let sec_end = size / SECTOR_SIZE + (if size % SECTOR_SIZE > 0 { 1 } else { 0 });
                uncovered.insert(0, sec_end);
            }
            else if let Some(s) = image_size && s != size {
                return Err(OpenError {
                    path: current_fn,
                    kind: OpenErrorKind::BadParentExtentDescriptorSize(
                        s, size
                    )
                });
            }

            // add the extents for this image to the span map
            for ex in img_extents {
                match &ex.storage {
                    ExtentStorage::Sparse(storage) => {
                        // Sparse storage is a collection of blocks of bytes.
                        // It need not cover the extent's whole space.

                        for &goff in storage.grain_table.keys() {
                            insert_span(
                                goff,
                                goff + storage.grain_size,
                                extents.len(),
                                &mut spans
                            );
                            remove_span(
                                goff,
                                goff + storage.grain_size,
                                &mut uncovered
                            );
                        }

                        extents.push(ex);
                        idx += 1;
                    },
                    ExtentStorage::Flat(_) => {
                        // Flat storage is a block of bytes.
                        // This extent will supply every range it has
                        // which isn't already covered.

                        insert_span(
                            ex.start_sector,
                            ex.start_sector + ex.sectors,
                            extents.len(),
                            &mut spans
                        );
                        remove_span(
                            ex.start_sector,
                            ex.start_sector + ex.sectors,
                            &mut uncovered
                        );

                        extents.push(ex);
                        idx += 1;
                    },
                    ExtentStorage::Zero => {
                        // Zero storage is a block of zeros.
                        // This extent will supply every range it has
                        // which isn't already covered.

                        insert_span(
                            ex.start_sector,
                            ex.start_sector + ex.sectors,
                            extents.len(),
                            &mut spans
                        );
                        remove_span(
                            ex.start_sector,
                            ex.start_sector + ex.sectors,
                            &mut uncovered
                        );

                        extents.push(ex);
                    }
                }

                // stop if we have extents for all spans
                if uncovered.is_empty() {
                    break 'img_loop size;
                }
            }

            // keep going if we are not at the end of the image chain
            match parent_fn {
                Some(parent_fn) => current_fn.set_file_name(parent_fn),
                None => break size
            }
        };

        if !uncovered.is_empty() {
            // TODO: fill missing spans with zeros?

            for u in &uncovered {
                eprintln!("uncovered [{},{})", u.0, u.1);
            }
        }

        let spans = spans.into_iter()
            .collect::<Vec<_>>();

        Ok(Self {
            image_path: image_path.as_ref().into(),
            image_size,
            spans,
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
        let beg = offset;

        // don't start reading past the end
        let image_end = self.image_size;
        if beg > image_end {
            return Err(ReadError::OffsetBeyondEnd(beg, self.image_size));
        }

        // limit the buffer to the image end
        if beg + buf.len() as u64 > image_end {
            buf = &mut buf[..(image_end - beg) as usize];
        }

        let mut i = match self.spans
            .binary_search_by_key(&(beg / SECTOR_SIZE), |e| e.0)
        {
            Ok(i) => i,
            // 0 is impossible as an insertion point because
            // there must be a span staring at 0
            Err(0) => unreachable!(),
            Err(i) => i - 1
        };

        let end = beg + buf.len() as u64;

        let span_count = self.spans.len();

        while offset < end {
            let span = self.spans.get(i).unwrap();

            let span_end = if i < span_count - 1 {
                span.1.0 * SECTOR_SIZE
            }
            else {
                image_end
            };

            let r = ((span_end - offset) as usize).min(buf.len());

            let ex = &mut self.extents[span.1.1];

            let r = match &mut ex.storage {
                &mut ExtentStorage::Sparse(ref mut storage) =>
                    read_sparse(offset, storage, &mut buf[..r])?,
                &mut ExtentStorage::Flat(ref mut storage) => {
                    let offset_in_extent = offset - ex.start_sector * SECTOR_SIZE;
                    read_flat(offset_in_extent, storage, &mut buf[..r])?
                },
                ExtentStorage::Zero => read_zero(&mut buf[..r])
            };

            offset += r as u64;
            buf = &mut buf[r..];

            if offset >= span_end {
                // advance to the next span to read more
                i += 1;
            }
        }

        Ok((end - beg) as usize)
    }
}

fn read_sparse(
    offset: u64,
    storage: &mut SparseStorage,
    mut buf: &mut [u8]
) -> Result<usize, ReadError>
{
    let grain_size = storage.grain_size * SECTOR_SIZE;
    let grain_index = offset / grain_size;
    let grain_data_offset = (offset % grain_size) as usize;

    buf = &mut buf[..(grain_size as usize - grain_data_offset)];

    match storage.grain_table.get(&grain_index) {
        None => {
            // last vmdk file, zero-fill
            buf.fill(0);
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

                buf.clone_from_slice(
                    &grain_data[grain_data_offset
                        ..grain_data_offset + buf.len()],
                );
            }
        }
    }

    Ok(buf.len())
}

fn read_flat(
    local_offset: u64,
    storage: &mut FlatStorage,
    buf: &mut [u8]
) -> Result<usize, ReadError>
{
    // FLAT, VMFS
    let f = &mut storage.file;
    // NB: only ExtentKind::Flat has nonzero offset
    f.seek(SeekFrom::Start(local_offset + storage.offset))?;
    f.read_exact(buf)?;
    Ok(buf.len())
}

fn read_zero(
    buf: &mut [u8]
) -> usize
{
    buf.fill(0);
    buf.len()
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
