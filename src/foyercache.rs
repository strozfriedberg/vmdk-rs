use async_trait::async_trait;
use foyer::{
    BlockEngineConfig,
    DefaultHasher,
    DeviceBuilder,
    FsDeviceBuilder,
    HybridCache,
    HybridCacheBuilder,
    HybridGetOrFetch
};
use foyer_common::code::HashBuilder;
use futures::future::{TryFutureExt, try_join_all};
use std::{
    fmt::Debug,
    sync::Arc
};
use tempfile::TempDir;
use tracing::{debug, trace};

use crate::{
    bytessource::BytesSource,
    cache::Cache,
    placeholdersource::PlaceholderSource
};

pub struct FoyerCache<S = DefaultHasher>
where
    S: HashBuilder + Debug
{
    chlen: usize,
    sources: Vec<Box<dyn BytesSource + Send>>,
    cache: Arc<HybridCache<(usize, u64), Vec<u8>, S>>,
    cache_dir: TempDir,
    readahead: usize
}

impl FoyerCache<DefaultHasher> {
    pub async fn with_default_cache(
        chlen: usize,
        mem_size: usize,
        disk_size: usize,
        readahead: usize
    ) -> Result<Self, std::io::Error>
    {
        let cache_dir = tempfile::tempdir()?;

        let device = FsDeviceBuilder::new(cache_dir.path())
            .with_capacity(disk_size)
            .build()
            .map_err(std::io::Error::other)?;

        let cache = HybridCacheBuilder::new()
            .memory(mem_size)
            .storage()
            .with_engine_config(BlockEngineConfig::new(device))
            .build()
            .await
            .map_err(std::io::Error::other)?;

        Ok(Self::new(cache, cache_dir, chlen, readahead))
    }
}

impl<S> FoyerCache<S>
where
    S: HashBuilder + Debug
{
    pub fn new(
        cache: HybridCache<(usize, u64), Vec<u8>, S>,
        cache_dir: TempDir,
        chlen: usize,
        readahead: usize
    ) -> Self
    {
        Self {
            chlen,
            sources: vec![],
            cache: Arc::new(cache),
            cache_dir,
            readahead
        }
    }
}

fn make_getter<S>(
    chlen: usize,
    idx: usize,
    source: &Box<dyn BytesSource + Send>,
    end: u64,
    cache: Arc<HybridCache<(usize, u64), Vec<u8>, S>>
) -> impl FnMut(u64) -> HybridGetOrFetch<(usize, u64), Vec<u8>, S>
where
    S: HashBuilder + Debug
{
    move |choff: u64| {
        let fetch = move ||
            source.read(choff, (choff + chlen as u64).min(end))
                .map_err(foyer::Error::io_error);

        trace!("fetching {idx} [{choff},{})", (choff + chlen as u64).min(end));
        cache.get_or_fetch(&(idx, choff), fetch)
    }
}

#[async_trait]
impl<S> Cache for FoyerCache<S>
where
    S: HashBuilder + Debug
{
    async fn read(
        &mut self,
        idx: usize,
        off: u64,
        buf: &mut [u8]
    ) -> Result<(), std::io::Error>
    {
        let source = self.sources.get(idx)
            .ok_or(std::io::Error::other(format!("{idx} out of bounds")))?;

        let end = source.end();

        // cs = chunk source
        let csbeg = (off / self.chlen as u64) * self.chlen as u64;
        let csend = off + buf.len() as u64;

        // ra = read-ahead
        let rabeg = csend.div_ceil(self.chlen as u64);
        let raend = (rabeg + (self.readahead * self.chlen) as u64).min(end);

        // request the chunks we need
        let getter = make_getter(
            self.chlen,
            idx,
            source,
            end,
            self.cache.clone()
        );

        let fut = try_join_all((csbeg..csend)
            .step_by(self.chlen)
            .map(getter)
        );

        // request read-ahead chunks in the background
        let getter = make_getter(
            self.chlen,
            idx,
            source,
            end,
            self.cache.clone()
        );

        let _ = (rabeg..raend).step_by(self.chlen)
            .map(getter)
            .map(tokio::spawn);

        // wait for the chunks we need
        let chunks = fut.await.map_err(std::io::Error::other)?;

        let mut bbeg = 0;

        for (choff, ch) in (csbeg..csend).step_by(self.chlen).zip(chunks) {
            trace!("fetched {idx} [{choff},{})", choff + ch.len() as u64);

            let chbeg = (off + bbeg) - choff;
            let chend = (chbeg + (buf.len() as u64 - bbeg)).min(ch.len() as u64);
            let bend = bbeg + (chend - chbeg);

            buf[bbeg as usize..bend as usize].copy_from_slice(&ch[chbeg as usize..chend as usize]);

            trace!("filled [{},{})", off + bbeg, off + bend);
            bbeg += chend - chbeg;
        }

        Ok(())
    }

    fn end(&self, idx: usize) -> Result<u64, std::io::Error> {
        self.sources.get(idx)
            .ok_or(std::io::Error::other(format!("{idx} out of bounds")))
            .map(|src| src.end())
    }

    fn add_source(&mut self, idx: usize, src: Box<dyn BytesSource + Send>) {
        if self.sources.len() <= idx {
            self.sources.resize_with(idx + 1, || Box::new(PlaceholderSource));
        }
        self.sources[idx] = src;
    }
}
