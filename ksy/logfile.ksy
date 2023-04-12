meta:
  id: log_file
  title: $LogFile file layout
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/files/logfile.html
# https://dfir.ru/2019/02/16/how-the-logfile-works/
# https://www.ntfs.com/transaction.htm

seq:
  - id: lfs_restart_page
    type: lfs_restart_page_t
    size: system_page_size
    process: update_seq_decoder
  - id: lfs_restart_page2
    type: lfs_restart_page_t
    size: system_page_size
    #TODO process (restore bytes) using usa_array_t
    process: update_seq_decoder

instances:
  system_page_size: # The size of each LFS_RESTART_PAGE, usually 4096
    pos: 0x10
    type: u4

  log_page_size:
    pos: 0x14
    type: u4

  record_pages:
    pos: system_page_size*2
    type: lfs_record_page_wrapper_t(log_page_size)
    size: log_page_size
    process: update_seq_decoder
    repeat: eos

types:
  usa_array_t:
    params:
    - id: size
      type: u2
    seq:
    - id: usa
      type: u2
      repeat: expr
      repeat-expr: size

  update_seq_array_t:
    seq:
    - id: update_seq_array_offset
      type: u2
    - id: update_seq_array_size
      type: u2

    instances:
      usa_array:
        pos: update_seq_array_offset
        type: usa_array_t(update_seq_array_size)

  lfs_restart_page_t:
    seq:
    - id: signature
      contents: RSTR
    - id: usa
      type: update_seq_array_t
    - id: chk_dsk_lsn
      type: u8
    - id: system_page_size
      type: u4
    - id: log_page_size
      type: u4
    - id: restart_offset
      type: u2
    - id: minor_version
      type: u2
    - id: major_version
      type: u2
    
    instances:
      chk_dsk_lsn_t:
        if: chk_dsk_lsn != 0
        type: lsn_t(chk_dsk_lsn)

      lfs_restart_area:
        pos: restart_offset
        type: lfs_restart_area_t
      
  lfs_restart_area_t:
    seq:
    - id: current_lsn
      type: u8
    - id: log_clients
      type: u2
    - id: client_free_list
      type: u2
    - id: client_in_use_list
      type: u2
    - id: flags
      type: u2
      enum: lfs_restart_area_flags_enum
    - id: seq_number_bits
      type: u4
    - id: restart_area_length
      type: u2
    - id: client_array_offset
      type: u2
    - id: file_size
      type: u8
    - id: last_lsn_data_length
      type: u4
    - id: record_header_length
      type: u2
    - id: log_page_data_offset
      type: u2
    - id: revision_number
      type: u4

    instances:
      current_lsn_t:
        type: lsn_t(current_lsn)

      clients:
        pos: _parent.restart_offset + client_array_offset
        type: lfs_client_record_t
        repeat: expr
        repeat-expr: log_clients

  lfs_client_record_t:
    seq:
    - id: oldest_lsn
      type: u8
    - id: client_restart_lsn
      type: u8
    - id: prev_client
      type: u2
    - id: next_client
      type: u2
    - id: seq_number
      type: u2
    - id: padding
      size: 6
    - id: client_name_length
      type: u4
    - id: client_name
      type: str
      encoding: UTF-16LE
      size: 128

    instances:
      oldest_lsn_t:
        if: oldest_lsn != 0
        type: lsn_t(oldest_lsn)
      
      client_restart_lsn_t:
        if: client_restart_lsn != 0
        type: lsn_t(client_restart_lsn)

  lsn_t:
    params:
    - id: lsn
      type: u8
  
    instances:

      abs_off:
        value: 8*(lsn & (1 << (64 - _root.lfs_restart_page.lfs_restart_area.seq_number_bits)) - 1)

      rel_off:
        value: abs_off % _root.log_page_size

      page_nr:
        value: abs_off / _root.log_page_size

      record_page:
        io: _root._io
        if: page_nr > 2 and _root.log_page_size * page_nr < _root.lfs_restart_page.lfs_restart_area.file_size
        pos: _root.log_page_size * page_nr
        type: lfs_record_page_wrapper_t(_root.log_page_size)

      record:
        io: _root._io
        if: rel_off != 0 and record_page.correct_signature and _root.log_page_size * page_nr + rel_off < _root.lfs_restart_page.lfs_restart_area.file_size
        pos: _root.log_page_size * page_nr + rel_off
        type: lfs_record_t

  unknown_t:
    params:
    - id: size
      type: u4
    seq:
    - id: data
      size: size

  lfs_record_page_wrapper_t:
    params:
    - id: size
      type: u4

    seq:
    - id: signature
      type: u4
    - id: record
      if: correct_signature
      type: lfs_record_page_t

    instances:
      correct_signature:
        value: signature == 0x44524352 # RCRD

  lfs_record_page_t:
    seq:
    - id: usa
      type: update_seq_array_t
    - id: last_lsn_or_file_offset
      type: u8
    - id: flags
      type: u4
      enum: lfs_record_page_flags_enum
    - id: page_count
      type: u2
    - id: page_position
      type: u2
    - id: next_record_offset
      type: u2
    - id: padding
      size: 6
    - id: last_end_lsn
      type: u8

    instances:
      last_lsn_t:
        if: last_lsn_or_file_offset != 0
        type: lsn_t(last_lsn_or_file_offset)

      last_end_lsn_t:
        if: last_end_lsn != 0
        type: lsn_t(last_end_lsn)

  lfs_record_t:
    seq:
    - id: this_lsn
      type: u8
    - id: client_previous_lsn
      type: u8
    - id: client_undo_next_lsn
      type: u8
    - id: client_data_length
      type: u4
    - id: client_seq_number
      type: u2
    - id: client_index
      type: u2
    - id: record_type
      type: u4
      enum: lfs_record_type_enum
    - id: transaction_id
      type: u4
    - id: flags
      type: u2
    - id: padding
      size: 6
    - id: data
      if: client_data_length > 0 and client_data_length < _root.log_page_size
      size: client_data_length
      type:
        switch-on: record_type
        cases:
          lfs_record_type_enum::client_record: ntfs_log_record_t
          lfs_record_type_enum::client_restart: ntfs_restart_area_t
          _: unknown_t(client_data_length)

    instances:
      client_previous_lsn_t:
        if: client_previous_lsn != 0
        type: lsn_t(client_previous_lsn)
      client_undo_next_lsn_t:
        if: client_undo_next_lsn != 0
        type: lsn_t(client_undo_next_lsn)

  ntfs_restart_area_t:
    seq:
    - id: major_version
      type: u4
    - id: minor_version
      type: u4
    - id: start_of_checkpoint_lsn
      type: u8
    - id: open_attribute_table_lsn
      type: u8
    - id: attribute_names_lsn
      type: u8
    - id: dirty_page_table_lsn
      type: u8
    - id: transaction_table_lsn
      type: u8
    - id: open_attribute_table_length
      type: u4
    - id: attribute_names_length
      type: u4
    - id: dirty_page_table_length
      type: u4
    - id: transaction_table_length
      type: u4
    - id: unknown1
      type: u8
    - id: previous_restart_record_lsn
      type: u8
    - id: bytes_per_cluster
      type: u4
    - id: padding
      type: u4
    - id: usn_journal
      type: u8
    - id: unknown2
      type: u8
    - id: unknown_lsn
      type: u8

    instances:
      start_of_checkpoint_lsn_t:
        if: start_of_checkpoint_lsn != 0
        type: lsn_t(start_of_checkpoint_lsn)

      open_attribute_table_lsn_t:
        if: open_attribute_table_lsn != 0
        type: lsn_t(open_attribute_table_lsn)

      attribute_names_lsn_t:
        if: attribute_names_lsn != 0
        type: lsn_t(attribute_names_lsn)

      dirty_page_table_lsn_t:
        if: dirty_page_table_lsn != 0
        type: lsn_t(dirty_page_table_lsn)

      transaction_table_lsn_t:
        if: transaction_table_lsn != 0
        type: lsn_t(transaction_table_lsn)

      previous_restart_record_lsn_t:
        if: previous_restart_record_lsn != 0
        type: lsn_t(previous_restart_record_lsn)

      unknown_lsn_t:
        if: unknown_lsn != 0
        type: lsn_t(unknown_lsn)

  ntfs_log_record_t:
    seq:
    - id: redo_op
      type: u2
      enum: ntfs_log_operation_enum
    - id: undo_op
      type: u2
      enum: ntfs_log_operation_enum
    - id: redo_off
      type: u2
    - id: redo_len
      type: u2
    - id: undo_off
      type: u2
    - id: undo_len
      type: u2
    - id: target_attr_off
      type: u2
    - id: lcns_follow
      type: u2
    - id: record_off
      type: u2
    - id: attr_off
      type: u2
    - id: cluster_off
      type: u2
    - id: reserved
      type: u2
    - id: target_vcn
      type: u8
    #- id: lcns_for_page
    
    instances:
      redo_data:
        if: redo_off != 0 and redo_len != 0
        pos: redo_off
        size: redo_len

      undo_data:
        if: undo_off != 0 and undo_len != 0
        pos: undo_off
        size: undo_len

enums:
  lfs_record_type_enum:
    0x0001: client_record
    0x0002: client_restart

  lfs_restart_area_flags_enum:
    0x0001: single_page_io
    0x0002: clean_dismount

  lfs_record_page_flags_enum:
    0x0001: record_end

  ntfs_log_operation_enum:
    0x00: noop
    0x01: compensationlogrecord
    0x02: initializefilerecordsegment
    0x03: deallocatefilerecordsegment
    0x04: writeendoffilerecordsegment
    0x05: createattribute
    0x06: deleteattribute
    0x07: updateresidentattributevalue
    0x08: updatenonresidentattributevalue
    0x09: updatemappingpairs
    0x0A: deletedirtyclusters
    0x0B: setnewattributesizes
    0x0C: addindexentrytoroot
    0x0D: deleteindexentryfromroot
    0x0E: addindexentrytoallocationbuffer
    0x0F: deleteindexentryfromallocationbuffer
    0x10: writeendofindexbuffer
    0x11: setindexentryvcninroot
    0x12: setindexentryvcninallocationbuffer
    0x13: updatefilenameinroot
    0x14: updatefilenameinallocationbuffer
    0x15: setbitsinnonresidentbitmap
    0x16: clearbitsinnonresidentbitmap
    0x17: hotfix
    0x18: endtoplevelaction
    0x19: preparetransaction
    0x1A: committransaction
    0x1B: forgettransaction
    0x1C: opennonresidentattribute
    0x1D: openattributetabledump
    0x1E: attributenamesdump
    0x1F: dirtypagetabledump
    0x20: transactiontabledump
    0x21: updaterecorddatainroot
    0x22: updaterecorddatainallocationbuffer
