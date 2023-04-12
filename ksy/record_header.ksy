meta:
  id: record_header
  title: $MFT's FILE Record
  imports:
    - symbols
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/concepts/file_record.html
seq:
  - id: magic 
    size: 4
    doc: The standard signature value is "FILE," but some entries will also have "BAAD" if chkdsk found an error in it.
  - id: offset_to_the_update_sequence
    type: u2
  - id: size_in_words_of_update_sequence
    doc: Number of entries in fixup array
    type: u2
  - id: lsn
    doc: $logfile sequence number
    type: u8
  - id: sequence_number
    type: u2
  - id: hard_link_count
    type: u2
  - id: offset_to_the_first_attribute
    type: u2
  - id: flags
    enum: symbols::record_header_enum
    type: u2
  - id: real_size_of_the_file_record
    doc: Used size of MFT entry
    type: u4
  - id: allocated_size_of_the_file_record
    type: u4
  - id: file_reference_to_the_base_file_record
    type: u8
  - id: next_attribute_id
    type: u2
  #- id: align_to_4_byte_boundary
  #  size: 2
  #- id: number_of_this_mft_record
  #  type: u4
  - id: update_sequence_number
    type: u2
  #- id: update_sequence_array
  #  size: 2 * size_in_words_of_update_sequence - 2
