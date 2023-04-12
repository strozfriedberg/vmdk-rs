meta:
  id: standard_information_attribute 
  title: Attribute - $STANDARD_INFORMATION (0x10)
  ks-version: 0.9
  tags:
    - ntfs
  imports:
    - filetime
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/standard_information.html

seq:
  - id: standard_information_attribute
    type: type(false) # TODO: ref on Version numbers https://flatcap.github.io/linux-ntfs/ntfs/attributes/volume_information.html 

types:
  timestamp_t:
    seq:
    - id: timestamp
      type: u8
    doc: Windows FILETIME format (number of 100-nanosecond intervals since January 1, 1601, UTC)

  type:
    params:
    - id: is_2k
      type: b1
    seq:
    - id: file_creation_time
      type: timestamp_t
    - id: file_altered_time
      type: timestamp_t
    - id: mft_changed_time
      type: timestamp_t
    - id: file_read_time
      type: timestamp_t
    - id: dos_file_permissions
      type: u4
      enum: file_permissions_enum
    - id: maximum_number_of_versions
      type: u4
    - id: version_number
      type: u4
    - id: class_id
      type: u4
    - id: owner_id
      type: u4
      if: is_2k
    - id: security_id
      type: u4
      if: is_2k
    - id: quota_charged
      type: u8
      if: is_2k
    - id: update_sequence_number
      type: u8
      if: is_2k

#instances:
  #...:
    #value:
enums:
  file_permissions_enum:
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
