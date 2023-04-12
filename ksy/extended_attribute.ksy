meta:
  id: extended_attribute
  title: Attribute - $EA (0xE0)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/ea.html

seq:
  - id: extended_attribute
    type: extended_attribute_t
    repeat: eos

types:
  extended_attribute_t:
    seq:
    - id: size
      type: u4
    - id: flags
      type: u1
      enum: extended_attribute_flags_enum
    - id: name_len
      type: u1
    - id: value_len
      type: u2
    - id: name
      type: str
      size: name_len
      encoding: UTF-8
    - id: value
      size: value_len
    - id: align
      size: size - name_len - value_len - 8

enums:
  extended_attribute_flags_enum:
    0x80: need_ca
