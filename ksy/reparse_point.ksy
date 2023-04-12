meta:
  id: reparse_point
  title: Attribute - $REPARSE_POINT (0xC0)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/reparse_point.html

seq:
  - id: reparse_point
    type: reparse_point_t

types:
  # https://www.tiraniddo.dev/2019/09/overview-of-windows-execution-aliases.html
  appexeclink_t:
    seq:
    - id: version
      type: u4
    - id: data
      type: str
      encoding: UTF-16LE
      # TODO split this string into 3
      # Package ID: <NUL Terminated Unicode String>
      # Entry Point: <NUL Terminated Unicode String>
      # Executable: <NUL Terminated Unicode String>
      size: _parent.data_len - 4 - 4
    - id: app_type
      type: str
      encoding: UTF-16LE
      size: 2

  data_buffer_t:
    seq:
    - id: data
      size: _parent.data_len

  mount_point_t:
    seq:
    - id: subst_name_offset
      type: u2
    - id: subst_name_len
      type: u2
    - id: print_name_offset
      type: u2
    - id: print_name_len
      type: u2
    
    instances:
      print_name:
        type: str
        pos: 16 + print_name_offset
        size: print_name_len
        encoding: UTF-16LE
      subs_name:
        type: str
        pos: 16 + subst_name_offset
        size: subst_name_len
        encoding: UTF-16LE

  symlink_t:
    seq:
    - id: subst_name_offset
      type: u2
    - id: subst_name_len
      type: u2
    - id: print_name_offset
      type: u2
    - id: print_name_len
      type: u2
    - id: flags
      type: u4
    
    instances:
      print_name:
        type: str
        pos: 20 + print_name_offset
        size: print_name_len
        encoding: UTF-16LE
      subs_name:
        type: str
        pos: 20 + subst_name_offset
        size: subst_name_len
        encoding: UTF-16LE

  reparse_point_t:
    seq:
    - id: type_and_flags
      type: u4
    - id: data_len
      type: u2
    - id: padding
      size: 2
    - id: reparse_data
      type:
        switch-on: flags
        cases:
          reparse_types_enum::symlink: symlink_t
          reparse_types_enum::mount_point: mount_point_t
          reparse_types_enum::appexeclink: appexeclink_t
          _: data_buffer_t
  
    instances:
      flags:
        pos: 0x0
        type: u4
        enum: reparse_types_enum
      is_alias:
        value: (type_and_flags & 0x20000000) != 0
      is_directory:
        value: (type_and_flags & 0x10000000) != 0
      is_high_latency:
        value: (type_and_flags & 0x40000000) != 0
      is_microsoft:
        value: (type_and_flags & 0x80000000) != 0

enums:
  reparse_types_enum:
    0x80000009: csv
    0x80000013: dedup
    0x8000000a: dfs
    0x80000012: dfsr
    0xc0000004: hsm
    0x80000006: hsm2
    0xa0000003: mount_point
    0x80000014: nfs
    0x80000007: sis
    0xa000000c: symlink
    0x80000008: wim
    0x80000016: dfm
    0x80000017: wof
    0x80000018: wci
    0x9000001a: cloud
    0x8000001b: appexeclink
    0x9000001c: gvfs
    0xa000001d: lx_symlink
    0x80000023: af_unix
    0x80000024: lx_fifo
    0x80000025: lx_chr
    0x80000026: lx_blk
