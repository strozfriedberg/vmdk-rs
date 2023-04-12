meta:
  id: mft_entry
  title: File - $MFT
  imports:
    - record_header
    - attribute_header
    - symbols
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/files/mft.html
seq:
  - id: record_header_body
    type: record_header
instances:
  attributes:
    pos: record_header_body.offset_to_the_first_attribute
#      size: record_header_body.real_size_of_the_file_record
    type: attributes_in_entry

types:
  timestamp_t:
    seq:
    - id: timestamp
      type: u8
    doc: Windows FILETIME format (number of 100-nanosecond intervals since January 1, 1601, UTC)

  standard_information_attribute:
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
      enum: symbols::file_permissions_enum
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

  attributes_in_entry:
    seq:
    - id: attribute_header
      type: attribute_header
      parent: _parent
    - id: attribute_body
      terminator: 0xff # 0xffffffff
      type: 
        switch-on: attribute_header.attr_type
        cases:
          symbols::attr_type_enum::standard_information: standard_information_attribute(is_2k)
          _: undefined_yet
    instances:
      is_2k:
        value: false  # TODO:...
      mft_entry_size:
        value: 1024 - sizeof<record_header> #- sizeof<attribute_header>

  undefined_yet:
    seq:
    - id: something
      size-eos: true
  
#  attributes_size: 
#    value: mft_entry_size - sizeof(record_header_t)

