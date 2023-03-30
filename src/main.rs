extern crate kaitai;

use std::ops::Deref;
use clap::Parser;
use generated::sparse_extent_header::*;
use generated::vmware_vmdk::*;

use self::kaitai::*;

pub mod generated;

#[derive(Parser)]
struct Cli {
    /// Path to disk image
    vmdk_path: String,
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_path: '{}'", cli.vmdk_path);

    let _io = BytesReader::open(cli.vmdk_path).unwrap();
    // let res: KResult<OptRc<SparseExtentHeader>> = SparseExtentHeader::read_into(&_io, None, None);
    // let header: OptRc<SparseExtentHeader>;
    let res: KResult<OptRc<VmwareVmdk>> = VmwareVmdk::read_into(&_io, None, None);
    let header: OptRc<VmwareVmdk>;

    match res {
        Ok(_) => { header = res.unwrap(); }
        Err(e) => { panic!("{:?}", e); }
    }

    println!("header: {header:?}");

    let mut i = 0;
    let h: VmwareVmdk = header.get().as_ref().to_owned();
    if let Ok(grains) = &h.grain_secondary() {
        let entries = grains.deref().as_slice();
        let (_, entries, _) = unsafe { entries.align_to::<u32>() };

        for entry in entries {
            println!("{entry} ({entry:#04X})");
            i += 1;
            if i > 20 {
                break;
            }
        }
    };
}
