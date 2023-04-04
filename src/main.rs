extern crate kaitai;

use std::ops::Deref;
use clap::Parser;
//use generated::sparse_extent_header::*;
use generated::vmware_vmdk::*;
use generated::mbr_partition_table::*;
use generated::gpt_partition_table::*;

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
            let part_type_value = *partition.partition_type();
            let partition_type: PartitionType = unsafe { std::mem::transmute(part_type_value) };
            let lba_start = *partition.lba_start() as usize;
            println!("status: {}, partition_type {:?} ({:02X}), num_sectors: {}, lba_start: {lba_start}",
                     partition.status(), partition_type, part_type_value, partition.num_sectors());

            match partition_type {
                PartitionType::PARTITION_SYSTEMID_EMPTY => {}
                PartitionType::PARTITION_SYSTEMID_LEGACY_MBR_EFI_HEADER => {
                    let gpt_part_offs = boot_sector_offs + lba_start * 0x200;
                    println!("gpt_part_offs: {gpt_part_offs:08X}");
                    let gpt_part_data = data.align_to::<[u8; 0x200]>(gpt_part_offs).to_vec();
                    let _io = BytesReader::from(gpt_part_data);
                    let gpt_part: OptRc<GptPartitionTable> = match GptPartitionTable::read_into(&_io, None, None) {
                        Ok(gpt) => gpt,
                        Err(e) => panic!("{:?}", e),
                    };
                    println!("sector_size {:?}", gpt_part.sector_size());
                    println!("primary {:?}", gpt_part.primary());
                    println!("backup {:?}", gpt_part.backup());
                    if let Ok(header) = &gpt_part.primary() {
                        let header = header.get();
                        println!("{header:?}");
                    };
                }
                _ => panic!("no implementation for {partition_type:?}"),
            }
        }
    };
}
