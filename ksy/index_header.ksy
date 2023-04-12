meta:
  id: index_header 
  title: index node header
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/concepts/node_header.html

seq:
  - id: first_index_entry
    type: u4
  - id: total_size_entries
    type: u4
  - id: allocated_size
    type: u4
  - id: flags
    type: u1
    enum: index_header_flags_enum
  - id: reserved
    size: 3
  
enums:
  index_header_flags_enum:
    0x01: non_leaf_node
