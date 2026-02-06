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

use crate::{
    bytessource::BytesSource,
    cache::Cache,
    cachereadseek::CacheReadSeek,
    descriptor::{read_descriptor_file, extract_parent_fn_hint},
    dummycache::DummyCache,
    errors::{InitError, OpenError, OpenErrorKind},
    foyercache::FoyerCache,
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

pub fn source_for_url(
    url: &Url,
    runtime: &Runtime
) -> Result<Box<dyn BytesSource + Send>, OpenError>
{
    match url.scheme() {
        "file" => {
            let p = if cfg!(windows) {
                // Windows file URLs get a spare / before the drive letter,
                // which we have to remove when using it as a path.
                url.path().trim_start_matches('/')
            }
            else {
                url.path()
            };

            let len = std::fs::metadata(p)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(p))?
                .len();
            Ok(Box::new(FileSource { path: p.into(), len }))
        },
        "s3" => {
            let name = url.host_str()
                .ok_or(OpenErrorKind::BadPath(url.to_string()))?;

            let bucket = *Bucket::new(
                name,
                Region::UsEast1,
                Credentials::anonymous().unwrap()
            )
            .map_err(std::io::Error::other)
            .map_err(OpenError::from)
            .map_err(|e| e.with_path(url))?;

            let key = url.path();

            let (h, code) = runtime.block_on(bucket.head_object(key))
                .map_err(std::io::Error::other)
                .map_err(OpenError::from)
                .map_err(|e| e.with_path(url))?;

            assert_eq!(code, 200);
            let len = h.content_length.unwrap().try_into().unwrap();
            debug!("content-length: {len}");

            Ok(Box::new(S3Source::new(bucket, key.into(), len)))
        },
        _ => Err(OpenErrorKind::UnsupportedScheme(url.to_string()).into())
    }
}

fn handle_image(
    current_url: &Url,
    mut idx: usize,
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>
) -> Result<(Vec<Extent>, Option<Url>), OpenError>
{
    let src = source_for_url(current_url, &runtime)?;
    let seg_len = src.end();

    cache.lock().unwrap().add_source(idx, src);

    let mut crs = CacheReadSeek::new(
        cache.clone(),
        runtime.clone(),
        idx,
        seg_len
    );

    idx += 1;

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
        current_url,
        &descriptor,
        header,
        cache.clone(),
        runtime.clone(),
        idx
    )?;

    let parent_url = extract_parent_fn_hint(&descriptor)
        .map(|p| current_url.join(&p)
            .map_err(|_| OpenErrorKind::BadPath(p))
        )
        .transpose()?;

    Ok((extents, parent_url))
}

impl VmdkReader {
    pub fn open<T: AsRef<str>>(
        image_path: T
    ) -> Result<Self, OpenError>
    {
        let mut current_url = path_or_url_to_url(&image_path)
            .ok_or(OpenErrorKind::BadPath(image_path.as_ref().into()))?;

        let runtime = Arc::new(
            tokio::runtime::Runtime::new()
                .map_err(InitError::TokioRuntimeFailed)
                .map_err(OpenErrorKind::from)?
        );

//        let c = DummyCache::new();

        let cache_chunk_size = 1024 * 1024;
//        let cache_mem_size = 1024;
//        let cache_disk_size = 256 * 1024 * 1024;
        let cache_mem_size = 256;
        let cache_disk_size = 256;
        let c = runtime.block_on(
            FoyerCache::with_default_cache(
                cache_chunk_size,
                cache_mem_size,
                cache_disk_size
            )
        )
        .map_err(InitError::CacheSetupFailed)
        .map_err(OpenErrorKind::from)?;

        let cache = Arc::new(Mutex::new(c));

        let mut idx = 0;
        let mut spans: BTreeMap<u64, (u64, usize)> = BTreeMap::new();
        let mut uncovered: BTreeMap<u64, u64> = BTreeMap::new();
        let mut extents = vec![];
        let mut image_size = None;

        let image_size = 'img_loop: loop {
            let (img_extents, parent_url) = handle_image(
                &current_url,
                idx,
                cache.clone(),
                runtime.clone()
            )?;

            idx += 1;

            // size for all images must match
            let size = img_extents.iter()
                .fold(0, |acc, i| acc + i.sectors) * SECTOR_SIZE;

            if image_size.is_none() {
                image_size = Some(size);
                let sec_end = size.div_ceil(SECTOR_SIZE);
                uncovered.insert(0, sec_end);
            }
            else if let Some(s) = image_size && s != size {
                return Err(OpenError {
                    path: current_url.as_ref().into(),
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
            let Some(parent_url) = parent_url else { break 'img_loop size; };
            current_url = parent_url;
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
}
