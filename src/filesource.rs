use futures::future::{BoxFuture, FutureExt};
use std::io::SeekFrom;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt}
};
use tracing::trace;

use crate::bytessource::BytesSource;

#[derive(Clone, Debug)]
pub struct FileSource {
    pub path: String,
    pub len: u64
}

impl BytesSource for FileSource {
    fn read(
        &self,
        beg: u64,
        end: u64
    ) -> BoxFuture<'static, Result<Vec<u8>, std::io::Error>>
    {
        let p = self.path.clone();

        async move {
            let mut r = File::open(p).await?;
            r.seek(SeekFrom::Start(beg)).await?;
            let mut buf = vec![0; (end - beg) as usize];
            r.read_exact(&mut buf[..]).await?;
            trace!("read [{beg},{end}) from File");
            Ok(buf)
        }.boxed()
    }

    fn end(&self) -> u64 {
        self.len
    }
}
