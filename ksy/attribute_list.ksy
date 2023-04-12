meta:
  id: attribute_list 
  title: Attribute - $ATTRIBUTE_LIST (0x20)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/attribute_list.html

seq:
  - id: attribute_list
    type: attribute_list_t
    repeat: eos

types:
  file_reference_t:
    seq:
    - id: seqment_low_part
      type: u4
    - id: seqment_high_part
      type: u2
    - id: sequence_number
      type: u2

  attribute_list_t:
    seq:
    - id: type
      type: u4
    - id: record_length
      type: u2
    - id: name_length
      type: u1
    - id: name_offset
      type: u1
    - id: starting_vcn
      type: u8
    - id: segment_ref
      type: file_reference_t
    - id: attribute_id
      type: u2
    - id: name
      type: str
      size: 2 * name_length
      encoding: UTF-16LE
    - id: padding
      size: record_length - name_offset - 2 * name_length
