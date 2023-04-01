extern crate kaitai;

use std::ops::Deref;
use clap::Parser;
//use generated::sparse_extent_header::*;
use generated::vmware_vmdk::*;
use generated::mbr_partition_table::*;

use self::kaitai::*;

pub mod generated;

pub mod partition_type;
use crate::partition_type::PartitionType;

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
        let boot_sector = data.align_to::<[u8; 0x200]>(boot_sector_offs).to_vec();
        let _io = BytesReader::from(boot_sector);
        let partition_table: OptRc<MbrPartitionTable> = match MbrPartitionTable::read_into(&_io, None, None) {
            Ok(pt) => pt,
            Err(e) => panic!("{:?}", e),
        };
        let partition_table = partition_table.get().as_ref().to_owned();
        for partition in &*partition_table.partitions() {
            let partition = partition.get();
            let partition_type: PartitionType = unsafe { std::mem::transmute(*partition.partition_type()) };
            // let partition_type = num::FromPrimitive::from_u8(*partition.partition_type());
            // let partition_type = if let Some(pt) = num::FromPrimitive::from_u8(*partition.partition_type()) {
            //     format!("{:?}", pt::<PartitionType>)
            // } else {
            //     format!("Unknown partition type {}", partition.partition_type())
            // };
            println!("status: {}, partition_type {:?}, num_sectors: {}", partition.status(), partition_type, partition.num_sectors());
        }
    };
}
