meta:
  id: volume_information_attribute 
  title: Attribute - $VOLUME_INFORMATION (0x70)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/volume_information.html

seq:
  - id: volume_information_attribute
    type: volume_information_attribute_t

types:
  volume_information_attribute_t:
    seq:
    - id: always_zero
      type: u8
    - id: version_number
      type: u2
      enum: version_number_enum
    - id: flags
      type: u2
      enum: volume_information_flags_enum

enums:
  version_number_enum:
    0x0103: windows_xp
    0x0003: windows_2000
    0x0201: windows_nt
  
  volume_information_flags_enum:
    0x0001: dirty
    0x0002: resize_logfile
    0x0004: upgrade_on_mount
    0x0008: mounted_on_nt4
    0x0010: delete_usn_underway
    0x0020: repair_object_ids
    0x8000: modified_by_chkdsk
