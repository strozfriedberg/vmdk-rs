use rand::Rng;
use sha1::{Digest, Sha1};

pub fn do_hash<RF>(
    reader: RF,
    image_size: u64,
    random_buf_size: bool
) -> String
where
    RF: Fn(u64, &mut [u8]) -> usize
{
    let mut hasher = Sha1::new();
    let mut buf: Vec<u8> = vec![0; 1048576];
    let mut offset = 0;

    while offset < image_size {
        let buf_size = if random_buf_size {
            rand::rng().random_range(0..buf.len())
        }
        else {
            buf.len()
        };

        let read = reader(offset, &mut buf[..buf_size]);

        if read == 0 {
            break;
        }

        hasher.update(&buf[..read]);

        offset += read as u64;
    }

    let result = hasher.finalize();
    format!("{:X}", result)
}
