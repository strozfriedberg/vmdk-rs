use std::{
    io::{Read, Seek, SeekFrom},
    sync::{Arc, Mutex}
};
use tokio::runtime::Runtime;

use crate::cache::Cache;

#[derive(Clone)]
pub struct CacheReadSeek {
    cache: Arc<Mutex<dyn Cache + Send>>,
    runtime: Arc<Runtime>,
    idx: usize,
    pos: u64
}

impl CacheReadSeek
{
    pub fn new(
        cache: Arc<Mutex<dyn Cache + Send>>,
        runtime: Arc<Runtime>,
        idx: usize,
        _len: u64
    ) -> Self {
        Self {
            cache,
            runtime,
            idx,
            pos: 0
        }
    }
}

impl Read for CacheReadSeek {
    fn read(
        &mut self,
        buf: &mut [u8]
    ) -> Result<usize, std::io::Error>
    {
        let mut cache = self.cache.lock().unwrap();

        // check that we don't read past the end of the source
        let send = cache.end(self.idx)?;
        let rend = send.min(self.pos + buf.len() as u64);

        let len = (rend - self.pos) as usize;

        if len > 0 {
            self.runtime.block_on(
                cache.read(self.idx, self.pos, &mut buf[..len])
            )?;
            self.pos = rend;
        }

        Ok(len)
    }
}

impl Seek for CacheReadSeek {
    fn seek(
        &mut self,
        pos: SeekFrom
    ) -> Result<u64, std::io::Error>
    {
        let end = self.cache.lock().unwrap().end(self.idx)?;

        let (base, offset) = match pos {
            SeekFrom::Start(n) => (n, 0),
            SeekFrom::End(n) => (end, n),
            SeekFrom::Current(n) => (self.pos, n)
        };

        self.pos = match base.checked_add_signed(offset) {
            Some(n) if n <= end => Ok(n),
            Some(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid seek past end"
            )),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position"
            ))
        }?;

        Ok(self.pos)
    }
}
