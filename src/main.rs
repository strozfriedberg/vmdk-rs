use clap::Parser;
use sha1::{Digest, Sha1};
use tracing_subscriber::{
    EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt
};

use vmdkrs::vmdk_reader::VmdkReader;

#[derive(Parser)]
struct Cli {
    /// Path to vmdk disk image
    vmdk_paths: Vec<String>,
}

fn do_hash(vmdk_path: &str) -> String /*hash*/ {
    let mut vmdk_reader = VmdkReader::open(vmdk_path).unwrap();
    let mut hasher = Sha1::new();
    let mut buf: Vec<u8> = vec![0; 1048576];
    let mut offset = 0;
    while offset < vmdk_reader.image_size {
        let buf_size = buf.len();
        let read = match vmdk_reader.read_at_offset(offset, &mut buf[..buf_size]) {
            Ok(v) => v,
            Err(e) => {
                panic!("{:?}", e);
            }
        };

        if read == 0 {
            break;
        }

        hasher.update(&buf[..read]);

        offset += read as u64;
    }
    let result = hasher.finalize();
    format!("{:X}", result)
}

fn main() {
    let stderr_layer = tracing_subscriber::fmt::layer()
//        .with_current_span(true)
        .without_time()
        .with_file(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .with_thread_names(false)
//        .with_target(false)
        .with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| {
                [
                    // log at info by default
                    "info",
                    // foyer is noisy below warn level
                    "foyer_memory=warn",
                    "foyer_storage=warn"
                ].join(",").into()
            })
        )
        .with(stderr_layer)
        .init();

    let cli = Cli::parse();
    let vmdk_paths: Vec<&str> = cli.vmdk_paths.iter().map(String::as_str).collect();
    for s in vmdk_paths {
        println!("{}: {}", s, do_hash(s));
    }
}
