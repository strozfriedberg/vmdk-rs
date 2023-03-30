# VMDK hosted binary extent header.
# this header is also used for monolithic flat images.

meta:
  id: sparse_extent_header
  title: The VMDK sparse extent data file
  tags:
    - vmdk
  endian: le
doc-ref: https://www.virtualbox.org/browser/vbox/trunk/src/VBox/Storage/VMDK.cpp
seq:
  - id: magic_number
    contents: "KDMV"
  - id: version
    type: u4
  - id: flags
    type: u4
  - id: capacity
    type: u8
  - id: grain_size
    type: u8
  - id: descriptor_offset
    type: u8
  - id: descriptor_size
    type: u8
  - id: num_gtes_per_gt
    type: u4
  - id: rgd_offset
    type: u8
  - id: gd_offset
    type: u8
  - id: over_head
    type: u8
  - id: unclean_shutdown
    type: b1
  - id: single_end_line_char
    contents: "\n"
  - id: non_end_line_char
    contents: " "
  - id: double_end_line_char1
    contents: "\r"
  - id: double_end_line_char2
    contents: "\n"
  - id: compress_algorithm
    type: u2
    enum: compression_method
  - id: pad
    size: 433
enums:
  compression_method:
    0x0: none
    0x1: deflate
