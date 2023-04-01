extern crate kaitai;

use std::ops::Deref;
use clap::Parser;
//use generated::sparse_extent_header::*;
use generated::vmware_vmdk::*;

use self::kaitai::*;

pub mod generated;

mod mmapper;
use crate::mmapper::MMapper;

#[derive(Parser)]
struct Cli {
    /// Path to disk image
    vmdk_path: String,
}

fn main() {
    let cli = Cli::parse();

    println!("vmdk_path: '{}'", cli.vmdk_path);

    let _io = BytesReader::open(&cli.vmdk_path).unwrap();
    // let res: KResult<OptRc<SparseExtentHeader>> = SparseExtentHeader::read_into(&_io, None, None);
    // let header: OptRc<SparseExtentHeader>;
    let res: KResult<OptRc<VmwareVmdk>> = VmwareVmdk::read_into(&_io, None, None);
    let header: OptRc<VmwareVmdk>;

    match res {
        Ok(_) => { header = res.unwrap(); }
        Err(e) => { panic!("{:?}", e); }
    }

//    println!("header: {header:?}");

    let data = MMapper::new(&cli.vmdk_path);
    let h: VmwareVmdk = header.get().as_ref().to_owned();
    println!("h: {h:?}");

    if let Ok(grains) = &h.grain_secondary() {
        let entries = grains.deref().as_slice();
        let (_, entries, _) = unsafe { entries.align_to::<u32>() };
        let offs = *data.align_to::<u32>((entries[0] as usize) << 9);
        let boot_sector_offs = (offs as usize) << 9;
        let boot_sector = data.align_to::<[u8; 0x200]>(boot_sector_offs);
        println!("{:02X} {:02X}", boot_sector[0x1FE], boot_sector[0x1FF]);
    };
}
