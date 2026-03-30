use futures::future::{BoxFuture, FutureExt};
use s3::{
    bucket::Bucket,
    creds::Credentials,
    region::Region,
    request::request_trait::ResponseData,
};
use tracing::trace;

use crate::bytessource::BytesSource;

pub struct S3Source {
    bucket: Bucket,
    path: String,
    len: u64
}

impl S3Source {
    pub fn new(
        bucket: Bucket,
        path: String,
        len: u64
    ) -> Self
    {
        Self {
            bucket,
            path,
            len
        }
    }

/*
    pub fn new(
        name: &str,
        region: &str,
        endpoint: &str,
        path: &str
    ) -> Self {
        let bucket = *Bucket::new(
            name,
            Region::Custom {
                region: region.into(),
                endpoint: endpoint.into()
            },
            Credentials::anonymous().unwrap()
        )
        .unwrap()
        .with_path_style();

        let (h, code) = bucket.head_object(path)
            .await
            .unwrap();

        assert_eq!(code, 200);
        let len = h.content_length.unwrap().try_into().unwrap();

        Self {
            bucket,
            path: path.into(),
            len
        }
    }
*/
}

impl BytesSource for S3Source {
    fn read(
        &self,
        beg: u64,
        end: u64
    ) -> BoxFuture<'static, Result<Vec<u8>, std::io::Error>>
    {
        let bucket = self.bucket.clone();
        let path = self.path.clone();

        async move {
            bucket.get_object_range(
                path,
                beg,
                Some(end - 1) // inclusive, augh!
            )
            .await
            .inspect(|_| trace!("read [{beg},{end}) from S3"))
            .map(ResponseData::to_vec)
            .map_err(std::io::Error::other)
        }.boxed()
    }

    fn end(&self) -> u64 {
        self.len
    }
}
