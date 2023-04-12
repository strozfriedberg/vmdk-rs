meta:
  id: symbols
  title: ntfs's flags and enums
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
#doc-ref: 
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

  attr_flags_enum:
    0x0001: compressed
    0x4000: encrypted
    0x8000: sparse
  attr_type_enum:  # doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/index.html
    0x10: standard_information
    0x20: attribute_list
    0x30: file_name
    0x40: object_id
    0x50: security_descriptor
    0x60: volume_name
    0x70: volume_information
    0x80: data
    0x90: index_root
    0xa0: index_allocation
    0xb0: bitmap
    0xc0: reparse_point
    0xd0: ea_information
    0xe0: ea
    0xf0: property_set
    0x100: logged_utility_stream
    
  record_header_enum:
    0x01:   record_is_in_use
    0x02:   record_is_a_directory # (filename index present)
    0x04:   record_is_an_exension # (set for records in the $extend directory)
    0x08:   special_index_present # (set for non-directory records containing an index: $secure, $objid, $quota, $reparse)
