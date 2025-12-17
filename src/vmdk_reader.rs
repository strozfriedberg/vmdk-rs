use s3::{
    bucket::Bucket,
    creds::Credentials,
    region::Region
};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    io::{self, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex}
};
use tokio::runtime::Runtime;
use tracing::debug;
use url::Url;

extern crate kaitai;

use crate::{
    bytessource::BytesSource,
    cache::Cache,
    cachereadseek::CacheReadSeek,
    descriptor::{read_descriptor_file, extract_parent_fn_hint},
    dummycache::DummyCache,
    errors::{InitError, OpenError, OpenErrorKind},
    filesource::FileSource,
    extents::{Extent, read_extents},
    header::{check_signature, read_header},
    s3source::S3Source,
    spans::{insert_span, remove_span},
    storage::ExtentStorage
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
                for (beg, end) in ex.spans() {
                    insert_span(beg, end, extents.len(), &mut spans);
                    remove_span(beg, end, &mut uncovered);
                }

                if ex.has_file() {
                    idx += 1;
                }

                extents.push(ex);

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

        // fill missing spans with zeros
        for (lb, ub) in uncovered {
            debug!("zero-filling uncovered span [{}, {})", lb, ub);

            let ex = Extent {
                start_sector: lb,
                sectors: ub - lb,
                storage: ExtentStorage::Zero
            };

            insert_span(lb, ub, extents.len(), &mut spans);

            extents.push(ex);
        }

        // spans are in bytes from here onward
        let spans = spans.into_iter()
            .map(|(lb, (ub, i))| (lb * SECTOR_SIZE, (ub * SECTOR_SIZE, i)))
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

        let end = beg + buf.len() as u64;

        let mut i = match self.spans.binary_search_by_key(&beg, |e| e.0) {
            Ok(i) => i,
            // 0 is impossible as an insertion point because
            // there must be a span staring at 0
            Err(0) => unreachable!(),
            Err(i) => i - 1
        };

        while offset < end {
            let span = self.spans[i];
            let span_end = span.1.0;
            let r = ((span_end - offset) as usize).min(buf.len());
            let ex = &mut self.extents[span.1.1];

            let r = ex.storage.read(offset, &mut buf[..r])?;

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

#[cfg(test)]
mod test {
    use super::*;
}
