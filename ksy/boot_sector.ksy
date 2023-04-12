meta:
  id: boot_sector
  title: $Boot metadata file
  ks-version: 0.9
  tags:
    - filesystem
  endian: le
seq:
  - id: jump_instruction
    size: 3
  - id: oem_id
    size: 8
  - id: bpb
    type: bpb_t
  - id: ebpb
    type: ebpb_t
types:
  bpb_t:
    seq:
    - id: byte_per_sector
      type: u2
    - id: sector_per_cluster
      type: u1
    - id: reserved_sectors
      type: u2
    - id: always0
      size: 3
    - id: unused1
      size: 2
    - id: media_descriptor
      type: u1
    - id: zeros
      size: 2
  ebpb_t:
    seq:
    - id: sector_per_track
      type: u2
    - id: number_of_heads
      type: u2
    - id: hidden_sectors
      type: u4
    - id: unused2
      size: 8
    - id: total_sectors
      type: u8
    - id: mft_cluster         # Logical Cluster Number for the file $MFT
      type: u8
    - id: mft_mirr_cluster    # Logical Cluster Number for the file $MFTMirr
      type: u8
    - id: clusters_per_file_record_segment
      type: u4
    - id: clusters_per_index_buffer
      type: u1
    - id: unused4
      size: 3
    - id: volume_serial_number
      type: u8
    - id: checksum
      type: u4
    - id: bootstrap_code
      size: 0x1aa
    - id: boot_signature
      contents: [0x55, 0xaa]
