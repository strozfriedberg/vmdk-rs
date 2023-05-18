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

fn do_hash(vmdk_path: &str) -> String /*hash*/ {
    let vmdk_reader = VmdkReader::open(&vmdk_path).unwrap();
    //println!("{vmdk_reader:?}");

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
    format!("{:X}", result)
}

fn do_hash_vmdk_dump(vmdk_paths: &[&str]) -> String {
    if cfg!(target_os = "windows") {
        let hash = Command::new("./tools/vmdk_dump")
            .args(vmdk_paths.iter().map(|a| a.replace("/", "\\")))
            .output()
            .expect("Failed to execute vmdk_dump");
        if !hash.status.success() {
            panic!("{}", String::from_utf8(hash.stderr).unwrap());
        }
        let hash = String::from_utf8(hash.stdout).unwrap();
        let hash = hash.split(" ").last().unwrap().trim();
        //println!("{hash:?}");
        hash.to_string()
    } else {
        "".to_string()
    }
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_paths: '{:?}'", cli.vmdk_paths);

    let vmdk_paths: Vec<&str> = cli.vmdk_paths.iter().map(String::as_str).collect();
    for (i, s) in vmdk_paths.iter().enumerate() {
        let h = do_hash(s);
        assert_eq!(h, do_hash_vmdk_dump(&vmdk_paths[i..]));
        println!("{}: {}", s, h);
    }
}

#[test]
fn test_all_images() {
    let do_hash2 = |vmdk_paths: &[&str]| {
        for (i, s) in vmdk_paths.iter().enumerate() {
            assert_eq!(do_hash(s), do_hash_vmdk_dump(&vmdk_paths[i..]));
        }
    };
    do_hash2(&vec!["data/vmfs_thick-000001.vmdk", "data/vmfs_thick.vmdk"]);
    do_hash2(&vec!["data/twoGbMaxExtentSparse.vmdk"]);
    do_hash2(&vec!["data/twoGbMaxExtentFlat.vmdk"]);
    do_hash2(&vec!["data/streamOptimized.vmdk"]);
    do_hash2(&vec!["data/monolithicSparse.vmdk"]);
    do_hash2(&vec!["data/monolithicFlat.vmdk"]);

    // vmdk_dump.exe crashes on this stream optimized image with markers
    assert_eq!(
        do_hash("data/streamOptimizedWithMarkers.vmdk"),
        "B6FD01DD1B93B3589E6D76F7507AF55C589EF69D"
    );
}
