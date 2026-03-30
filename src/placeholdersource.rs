use futures::future::BoxFuture;

use crate::bytessource::BytesSource;

pub struct PlaceholderSource;

impl BytesSource for PlaceholderSource {
    fn read(
        &self,
        _beg: u64,
        _end: u64
    ) -> BoxFuture<'static, Result<Vec<u8>, std::io::Error>>
    {
        unreachable!();
    }

    fn end(&self) -> u64 {
        unreachable!();
    }
}
