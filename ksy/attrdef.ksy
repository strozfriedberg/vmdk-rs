meta:
  id: attrdef 
  title: Attribute definitions - $AttrDef file layout
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/files/attrdef.html

seq:
  - id: attrdef
    type: attrdef_t
    repeat: eos

types:
  attrdef_t:
    seq:
    - id: name
      type: str
      size: 128
      encoding: UTF-16LE
    - id: type
      type: u4
    - id: display_rule
      type: u4
    - id: collation_rule
      type: u4
      enum: collation_rule_enum
    - id: flags
      type: u4
      enum: flags_enum
    - id: min_size
      type: u8
    - id: max_size
      type: u8
  
enums:
  collation_rule_enum:
    0x00: binary
    0x01: filename
    0x02: unicode_string
    0x10: unsigned_long
    0x11: sid
    0x12: security_hash
    0x13: multiple_unsigned_longs

  flags_enum:
    0x02: indexed
    0x40: resident
    0x80: non_resident
