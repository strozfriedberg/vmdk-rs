use bytesize::ByteSize;
use clap::Parser;
use sha1::{Digest, Sha1};
use std::{
    process::ExitCode,
    time::{Duration, Instant}
};
use tracing_subscriber::{
    EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt
};

use vmdkrs::vmdk_reader::{VmdkError, VmdkReader};

#[derive(Parser)]
struct Args {
    /// Path to vmdk disk image
    vmdk_paths: Vec<String>
}

fn display_progress(
    offset: u64,
    image_size: u64,
    image_size_bs_disp: &bytesize::Display,
    start: Instant
) {
    let offset_bs = ByteSize::b(offset);
    eprintln!(
        "{:.1}/{:.1} = {:.1}%, {:.1}MiB/s",
        offset_bs.display().iec(),
        image_size_bs_disp,
        offset as f32 / image_size as f32 * 100.0,
        offset_bs.as_mib() / start.elapsed().as_secs_f64()
    );
}

fn do_hash<P>(path: P) -> Result<Vec<u8>, VmdkError>
where
    P: AsRef<str>
{
    let mut vmdk_reader = VmdkReader::open(path.as_ref())?;
    let mut hasher = Sha1::new();
    let mut buf: Vec<u8> = vec![0; 1048576];
    let mut offset = 0;

    let image_size_bs_disp = ByteSize::b(vmdk_reader.image_size)
        .display()
        .iec();

    let mut prev_prog = Instant::now();
    let start = prev_prog;

    while offset < vmdk_reader.image_size {
        let buf_size = buf.len();
        let read = vmdk_reader.read_at_offset(offset, &mut buf[..buf_size])?;

        if read == 0 {
            break;
        }

        hasher.update(&buf[..read]);

        offset += read as u64;

        if prev_prog.elapsed() > Duration::from_secs(2) {
            display_progress(
                offset,
                vmdk_reader.image_size,
                &image_size_bs_disp,
                start
            );
            prev_prog = Instant::now();
        }
    }

    display_progress(
        offset,
        vmdk_reader.image_size,
        &image_size_bs_disp,
        start
    );

    Ok(hasher.finalize().to_vec())
}

fn run(args: Args) -> Result<(), VmdkError> {
    for p in &args.vmdk_paths {
        println!("{p}: {}", hex::encode(do_hash(p)?));
    }
    Ok(())
}

fn main() -> ExitCode {
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
                    "foyer=warn",
                    "foyer_memory=warn",
                    "foyer_storage=warn"
                ].join(",").into()
            })
        )
        .with(stderr_layer)
        .init();

    let args = Args::parse();

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::FAILURE
        }
    }
}
