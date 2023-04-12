meta:
  id: index_allocation_buffer 
  title: INDEX_ALLOCATION_BUFFER
  tags:
    - ntfs
  imports:
    - index_header
  endian: le

seq:
  - id: index_allocation_buffer
    type: index_allocation_buffer_t

types:
  index_allocation_buffer_t:
    seq:
    - id: signature
      contents: [0x49, 0x4e, 0x44, 0x58]
    - id: update_seq_array_offset
      type: u2
    - id: update_seq_array_size
      type: u2
    - id: lsn
      type: u8
    - id: this_block
      type: u8
    - id: index_header
      type: index_header

enums:
  index_header_flags_enum:
    0x01: non_leaf_node
