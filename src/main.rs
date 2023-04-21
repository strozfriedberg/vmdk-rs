extern crate kaitai;

use clap::Parser;

pub mod generated;
pub mod vmdk_reader;
use sha1::{Digest, Sha1};
use vmdk_reader::VmdkReader;

mod mmapper;

// use std::fs::File;
// use std::io::Write;

#[derive(Parser)]
struct Cli {
    /// Path to vmdk disk image
    vmdk_path: String,
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_path: '{}'", cli.vmdk_path);

    let vmdk_reader = VmdkReader::open(&cli.vmdk_path).unwrap();
    println!("{vmdk_reader:?}");

    let mut hasher = Sha1::new();
    let mut buf: Vec<u8> = vec![0; 1048576];
    let mut offset = 0;
    while offset < vmdk_reader.total_size {
        let buf_size = buf.len();
        let readed = match vmdk_reader.read_at_offset(offset, &mut buf[..buf_size]) {
            Ok(v) => v,
            Err(e) => {
                panic!("{:?}", e);
            }
        };

        hasher.update(&buf[..readed]);

        offset += readed as u64;
    }
    let result = hasher.finalize();
    println!("{} {:X}\n", cli.vmdk_path, result);
}
