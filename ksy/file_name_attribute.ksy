meta:
  id: file_name_attribute 
  title: Attribute - $FILE_NAME (0x30)
  ks-version: 0.9
  tags:
    - ntfs
  imports:
    - filetime
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/file_name.html

seq:
  - id: file_name_attribute
    type: file_name_attribute_t 

types:
  timestamp_t:
    seq:
    - id: timestamp
      type: u8
    doc: Windows FILETIME format (number of 100-nanosecond intervals since January 1, 1601, UTC)

  file_name_attribute_t:
    seq:
    - id: directory_file_reference_number
      type: u8
    - id: file_creation_time
      type: timestamp_t
    - id: file_altered_time
      type: timestamp_t
    - id: mft_changed_time
      type: timestamp_t
    - id: file_read_time
      type: timestamp_t
    - id: allocated_size
      type: u8
    - id: real_size
      type: u8
    - id: flags
      type: u4
      enum: file_flags_enum
    - id: alignment_or_reserved
      type: u4
    - id: name_length
      type: u1
    - id: name_type
      type: u1
      enum: file_name_type_enum
    - id: name
      type: str
      size: 2 * name_length
      encoding: UTF-16LE

enums:
  file_name_type_enum:
    0x00: posix
    0x01: win32
    0x02: dos
    0x03: win32_dos
  
  file_flags_enum:
    0x0001: read_only
    0x0002: hidden
    0x0004: system
    0x0020: archive
    0x0040: device
    0x0080: normal
    0x0100: temporary
    0x0200: sparse_file
    0x0400: reparse_point
    0x0800: compressed
    0x1000: offline
    0x2000: not_content_indexed
    0x4000: encrypted
    0x10000000: directory
    0x20000000: index_view
