extern crate kaitai;

use clap::Parser;
use std::fs::File;
use std::io::prelude::*;

pub mod generated;
pub mod vmdk_reader;
use vmdk_reader::VmdkReader;

mod mmapper;

// use std::fs::File;
// use std::io::Write;

#[derive(Parser)]
struct Cli {
    /// Path to disk image
    vmdk_path: String,
    /// Path to dump file (will be created)
    dump_path: String,
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_path: '{}'", cli.vmdk_path);

    let vmdk_reader = VmdkReader::open(&cli.vmdk_path).unwrap();
    println!("{vmdk_reader:?}");

    let mut file = File::create(cli.dump_path.clone()).unwrap();
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
        file.write_all(&buf[..readed]).unwrap();
        offset += readed as u64;
    }
}
