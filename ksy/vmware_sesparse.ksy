meta:
  id: vmware_sesparse
  file-extension: vmdk
  endian: le
doc-ref: https://lists.nongnu.org/archive/html/qemu-block/2019-06/msg00932.html
seq:
  - id: magic
    type: u8
  - id: version
    type: u8
  - id: capacity
    type: u8
  - id: grain_size
    type: u8
  - id: grain_table_size
    type: u8
  - id: flags
    type: u8
  - id: reserved1
    type: u8
  - id: reserved2
    type: u8
  - id: reserved3
    type: u8
  - id: reserved4
    type: u8
  - id: volatile_header_offset
    type: u8
  - id: volatile_header_size
    type: u8
  - id: journal_header_offset
    type: u8
  - id: journal_header_size
    type: u8
  - id: journal_offset
    type: u8
  - id: journal_size
    type: u8
  - id: grain_dir_offset
    type: u8
  - id: grain_dir_size
    type: u8
  - id: grain_tables_offset
    type: u8
  - id: grain_tables_size
    type: u8
  - id: free_bitmap_offset
    type: u8
  - id: free_bitmap_size
    type: u8
  - id: backmap_offset
    type: u8
  - id: backmap_size
    type: u8
  - id: grains_offset
    type: u8
  - id: grains_size
    type: u8
