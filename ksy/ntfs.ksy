meta:
  id: ntfs
  title: The Windows NT file system (NTFS)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
  imports:
    - boot_sector
    - mft_entry

seq:
  - id: boot
    type: boot_sector

instances:
  bytes_per_sector:
    value: boot.bpb.byte_per_sector
  bytes_per_cluster:
    value: boot.bpb.sector_per_cluster * bytes_per_sector
  cpfrs:
    value: (boot.ebpb.clusters_per_file_record_segment).as<s4>
  bytes_per_frs:
    value: 'cpfrs < 0 ? 1 << -cpfrs : bytes_per_cluster * cpfrs'
  number_of_sectors:
    value: boot.ebpb.total_sectors
  extent_length:
    value: (number_of_sectors * bytes_per_sector).as<u8>
  mft_offset_0:
    value: (boot.ebpb.mft_cluster * bytes_per_cluster).as<u8>
  mft_mirror_offset_0:
    value: (boot.ebpb.mft_mirr_cluster * bytes_per_cluster).as<u8>
  mft_offset_1:
    value: (mft_offset_0 + bytes_per_frs).as<u8>
  mft_mirror_offset_1:
    value: (mft_mirror_offset_0 + bytes_per_frs).as<u8>
  mft_offset_2:
    value: (mft_offset_1 + bytes_per_frs).as<u8>
  mft_mirror_offset_2:
    value: (mft_mirror_offset_1 + bytes_per_frs).as<u8>
  mft_offset_3:
    value: (mft_offset_2 + bytes_per_frs).as<u8>
  mft_mirror_offset_3:
    value: (mft_mirror_offset_2 + bytes_per_frs).as<u8>

  mft:
    pos: mft_offset_0
    type: mft_entry
