meta:
  id: attribute_header
  title: standard attribute header
  imports:
    - symbols
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/concepts/attribute_header.html
seq:
  - id: type
    type: u4
    enum: symbols::attr_type_enum
  - id: length                    # including this header
    type: u4
  - id: non_resident_flag         # resident -> 0x00, non-resident -> 0x01
    type: u1
  - id: name_length
    type: u1
  - id: offset_to_the_name        # no name -> 0; resident, named -> 0x18; non-resident, named -> 0x40
    type: u2
  - id: flags
    type: u2
    enum: symbols::attr_flags_enum
  - id: id
    type: u2
  - id: resident_part
    type: resident_part
    if: non_resident_flag == 0x00
  - id: non_resident_part
    type: non_resident_part
    if: non_resident_flag == 0x01

types:
  resident_part:
    seq:
    - id: length_of_the_attribute
      type: u4
    - id: offset_to_the_attribute             # 2 * length_of_the_attribute + 0x18, rounded up to a multiple of 4 bytes
      type: u2
      #value: (offset_to_the_attribute + 1) % 0x100000000
    - id: indexed_flag
      type: u1
    - id: padding
      size: 1
    - id: name1
      type: str
      size: 2 * _parent.name_length
      encoding: UTF-16LE
    - id: the_attribute
      size: length_of_the_attribute

  non_resident_part:
    seq:
    - id: starting_vcn
      type: u8
    - id: last_vcn
      type: u8
    - id: offset_to_the_data_runs             # 2 * name_length + 0x40, rounded up to a multiple of 4 bytes
      type: u2
      #value: (offset_to_the_data_runs + 1) % 0x100000000
    - id: compression_unit_size               # compression unit size = 2x clusters. 0 implies uncompressed
      type: u2
    - id: padding1
      size: 4
    - id: allocated_size_of_the_attribute     # this is the attribute size rounded up to the cluster size
      type: u8
    - id: real_size_of_the_attribute
      type: u8
    - id: initialized_data_size_of_the_stream # compressed data size
      type: u8
    - id: name2
      type: str
      size: 2 * _parent.name_length
      encoding: UTF-16LE
    #- id: data_runs
    #  size: ?
