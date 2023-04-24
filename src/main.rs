extern crate kaitai;

use clap::Parser;

pub mod generated;
pub mod vmdk_reader;
use sha1::{Digest, Sha1};
use vmdk_reader::VmdkReader;

mod mmapper;

use std::process::Command;

#[derive(Parser)]
struct Cli {
    /// Path to vmdk disk image
    vmdk_paths: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_paths: '{:?}'", cli.vmdk_paths);

    for vmdk_path in &cli.vmdk_paths  {
        let vmdk_reader = VmdkReader::open(vmdk_path).unwrap();
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
        println!("{} {:X}\n", vmdk_path, result);

        let hash = Command::new("./tools/vmdk_dump")
            .arg(vmdk_path.replace("/", "\\"))
            .output()
            .expect("Failed to execute vmdk_dump");
        let hash = String::from_utf8(hash.stdout).unwrap();
        let hash = hash.split(" ").skip(1).next().unwrap().trim();
        //println!("{hash:?}");
        assert_eq!(&format!("{result:X}"), hash);
    }
}
