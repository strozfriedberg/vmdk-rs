meta:
  id: index_entry 
  title: index entry
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/concepts/index_entry.html

seq:
  - id: index_entry
    type: index_entry_t

types:
  index_entry_t:
    seq:
    - id: file_ref
      size: 8
    - id: length
      type: u2
    - id: attribute_length
      type: u2
    - id: flags
      type: u2
      enum: index_entry_flags_enum

enums:
  index_entry_flags_enum:
    0x01: entry_node
    0x02: entry_end
