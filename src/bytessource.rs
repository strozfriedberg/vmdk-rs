use futures::future::BoxFuture;

pub trait BytesSource {
    fn read(
        &self,
        beg: u64,
        end: u64
    ) -> BoxFuture<'static, Result<Vec<u8>, std::io::Error>>;

    fn end(&self) -> u64;
}
