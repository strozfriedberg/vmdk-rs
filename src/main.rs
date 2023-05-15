extern crate kaitai;

use clap::Parser;

pub mod generated;
pub mod vmdk_reader;
use sha1::{Digest, Sha1};
use vmdk_reader::VmdkReader;

use std::process::Command;

#[derive(Parser)]
struct Cli {
    /// Path to vmdk disk image
    vmdk_paths: Vec<String>,
}

fn do_hash(vmdk_paths: &Vec<&str>) {
    for i in 0..vmdk_paths.len() {
        let vmdk_reader = VmdkReader::open(&vmdk_paths[i]).unwrap();
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

            if readed == 0 {
                break;
            }

            hasher.update(&buf[..readed]);

            offset += readed as u64;
        }
        let result = hasher.finalize();
        println!("{} {:X}\n", vmdk_paths[i], result);

        if cfg!(target_os = "windows") {
            let hash = Command::new("./tools/vmdk_dump")
                .args(vmdk_paths[i..].iter().map(|a| a.replace("/", "\\")))
                .output()
                .expect("Failed to execute vmdk_dump");
            if !hash.status.success() {
                panic!("{}", String::from_utf8(hash.stderr).unwrap());
            }
            let hash = String::from_utf8(hash.stdout).unwrap();
            let hash = hash.split(" ").last().unwrap().trim();
            //println!("{hash:?}");
            assert_eq!(&format!("{result:X}"), hash);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_paths: '{:?}'", cli.vmdk_paths);

    do_hash(&cli.vmdk_paths.iter().map(String::as_str).collect());
}

#[test]
fn test_all_images() {
    do_hash(&vec!["data/vmfs_thick-000001.vmdk", "data/vmfs_thick.vmdk"]);
    do_hash(&vec!["data/twoGbMaxExtentSparse.vmdk"]);
    do_hash(&vec!["data/twoGbMaxExtentFlat.vmdk"]);
    do_hash(&vec!["data/streamOptimized.vmdk"]);
    do_hash(&vec!["data/monolithicSparse.vmdk"]);
    do_hash(&vec!["data/monolithicFlat.vmdk"]);
}
