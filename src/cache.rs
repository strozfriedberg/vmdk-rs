use async_trait::async_trait;

use crate::bytessource::BytesSource;

#[async_trait]
pub trait Cache {
    async fn read(
        &mut self,
        idx: usize,
        off: u64,
        buf: &mut [u8]
    ) -> Result<(), std::io::Error>;

    fn end(&self, idx: usize) -> Result<u64, std::io::Error>;

    fn add_source(&mut self, idx: usize, src: Box<dyn BytesSource + Send>);
}
