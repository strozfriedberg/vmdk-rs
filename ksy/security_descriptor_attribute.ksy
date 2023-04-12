meta:
  id: security_descriptor_attribute
  title: Attribute - $SECURITY_DESCRIPTOR (0x50)
  ks-version: 0.9
  tags:
    - ntfs
  endian: le
doc-ref: https://github.com/libyal/libfwnt/blob/main/documentation/Security%20Descriptor.asciidoc
#doc-ref: https://flatcap.github.io/linux-ntfs/ntfs/attributes/security_descriptor.html

seq:
  - id: security_attribute_header
    type: security_attribute_header_t

types:
  security_attribute_header_t:
    seq:
    - id: revision      # 0x1 for now
      type: u1
    - id: padding
      type: u1
    - id: control_flags # usually 0x4 (dacl present), or 0x14 (dacl present + sacl present). see flags
      type: u2
      enum: control_flags_enum
    - id: offset_to_user_sid
      type: u4
    - id: offset_to_group_sid
      type: u4
    - id: offset_to_sacl
      type: u4
    - id: offset_to_dacl
      type: u4

    instances:
      user_sid:
        pos: offset_to_user_sid
        type: sid_t

      group_sid:
        pos: offset_to_group_sid
        type: sid_t

      sacl: # system access control list
        pos: offset_to_sacl
        if: offset_to_sacl != 0
        type: acl_t

      dacl: # discretionary access control list
        pos: offset_to_dacl
        if: offset_to_dacl != 0
        type: acl_t

 # Access Control List
  acl_t:
    seq:
    - id: revision
      type: u1
    - id: padding
      type: u1
    - id: acl_size
      type: u2
    - id: ace_count
      type: u2
    - id: padding2
      type: u2
    - id: ace
      type: ace_t
      repeat: expr
      repeat-expr: ace_count

  # Access Control Entries
  ace_t:
    seq:
    - id: type
      type: u1
      enum: ace_types_enum
    - id: flags
      type: u1
      enum: ace_flags_enum
    - id: size
      type: u2
    - id: ace
      type:
        switch-on: type
        cases:
          ace_types_enum::access_allowed:                 ace_basic_t
          ace_types_enum::access_denied:                  ace_basic_t
          ace_types_enum::system_audit:                   ace_basic_t
          ace_types_enum::system_alarm:                   ace_basic_t
          ace_types_enum::access_allowed_compound:        ace_unknown_t(size-4)
          ace_types_enum::access_allowed_object:          ace_object_t
          ace_types_enum::access_denied_object:           ace_object_t
          ace_types_enum::system_audit_object:            ace_object_t
          ace_types_enum::system_alarm_object:            ace_object_t
          ace_types_enum::access_allowed_callback:        ace_basic_t
          ace_types_enum::access_denied_callback:         ace_basic_t
          ace_types_enum::access_allowed_callback_object: ace_object_t
          ace_types_enum::access_denied_callback_object:  ace_object_t
          ace_types_enum::system_audit_callback:          ace_basic_t
          ace_types_enum::system_alarm_callback:          ace_basic_t
          ace_types_enum::system_audit_callback_object:   ace_object_t
          ace_types_enum::system_alarm_callback_object:   ace_object_t
          ace_types_enum::system_mandatory_label:         ace_basic_t
          _: ace_unknown_t(size-4)

  ace_unknown_t:
    params:
    - id: size
      type: u2
    seq:
    - id: data
      size: size

  ace_basic_t:
    seq:
    - id: access_mask
      type: access_mask_rights_t
    - id: sid
      type: sid_t

  ace_object_t:
    seq:
    - id: access_mask
      type: access_mask_rights_t
    - id: flags
      type: u4
      enum: object_ace_flags_enum_t
    - id: object_type_class_id
      size: 16 # GUID
      if: flags == object_ace_flags_enum_t::ace_object_type_present
    - id: inherited_object_type_class_id
      size: 16 # GUID
      if: flags == object_ace_flags_enum_t::ace_inherited_object_type_present
    - id: sid
      type: sid_t

  access_mask_rights_t:
    seq:
    - id: object_specific_access_rights
      type: b16
      enum: object_specific_access_rights_enum
    - id: standard_access_rights
      type: b7
      enum: standard_access_types_enum
    - id: can_access_security_acl
      type: b1
    - id: reserved
      type: b4
    - id: generic_all__read_write_execute
      type: b1
    - id: generic_execute
      type: b1
    - id: generic_write
      type: b1
    - id: generic_read
      type: b1

  sid_t:
    seq:
    - id: revision
      type: u1
    - id: sub_authority_count
      type: u1
    - id: authority
      size: 6
    - id: sub_authority
      type: sub_authority_t
      repeat: expr
      repeat-expr: sub_authority_count

  sub_authority_t:
    seq:
    - id: authority
      size: 4

enums:
  object_ace_flags_enum_t:
    0x01: ace_object_type_present
    0x02: ace_inherited_object_type_present

  object_specific_access_rights_enum:
    0x001: file_read_data
    0x002: file_write_data
    0x004: file_append_data
    0x008: file_read_ea
    0x010: file_write_ea
    0x020: file_execute
    0x040: file_delete_child
    0x080: file_read_attirbutes
    0x100: file_write_attributes
    0x200: fsdright_write_own_property
    0x400: fsdright_delete_own_item
    0x800: fsdright_view_item

  standard_access_types_enum:
    0x01: delete
    0x02: read_control
    0x04: write_dac
    0x08: write_owner
    0x10: synchronize

  ace_types_enum:
    0x00: access_allowed
    0x01: access_denied
    0x02: system_audit
    0x03: system_alarm
    0x04: access_allowed_compound
    0x05: access_allowed_object
    0x06: access_denied_object
    0x07: system_audit_object
    0x08: system_alarm_object
    0x09: access_allowed_callback
    0x0a: access_denied_callback
    0x0b: access_allowed_callback_object
    0x0c: access_denied_callback_object
    0x0d: system_audit_callback
    0x0e: system_alarm_callback
    0x0f: system_audit_callback_object
    0x10: system_alarm_callback_object
    0x11: system_mandatory_label

  ace_flags_enum:
    0x01: object_inherits_ace
    0x02: container_inherits_ace
    0x04: do_not_propagate_inherit_ace
    0x08: inherit_only_ace
    0x40: audit_on_success
    0x80: audit_on_failure

  control_flags_enum:
    0x0001: owner_defaulted
    0x0002: group_defaulted
    0x0004: dacl_present
    0x0008: dacl_defaulted
    0x0010: sacl_present
    0x0020: sacl_defaulted
    0x0100: dacl_auto_inherit_req
    0x0200: sacl_auto_inherit_req
    0x0400: dacl_auto_inherited
    0x0800: sacl_auto_inherited
    0x1000: dacl_protected
    0x2000: sacl_protected
    0x4000: rm_control_valid
    0x8000: self_relative
