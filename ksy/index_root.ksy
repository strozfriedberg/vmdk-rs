meta:
  id: index_root 
  title: Attribute - $INDEX_ROOT (0x90)
  tags:
    - ntfs
  imports:
    - index_header
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/index_root.html

seq:
  - id: index_root
    type: index_root_t

types:
  index_root_t:
    seq:
    - id: attribute_type
      type: u4
    - id: collation_rule
      type: u4
    - id: bytes_per_index_record
      type: u4
    - id: blocks_per_index_record
      type: u1
    - id: reserved
      size: 3
    - id: index_header
      type: index_header
