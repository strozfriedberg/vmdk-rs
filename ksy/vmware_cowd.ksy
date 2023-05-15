meta:
  id: vmware_cowd
  endian: le
doc-ref: 'https://github.com/libyal/libvmdk/blob/main/documentation/VMWare%20Virtual%20Disk%20Format%20(VMDK).asciidoc#51-file-header'
seq:
  - id: magic
    contents: "COWD"
  - id: version
    type: u4
  - id: flags
    type: u4
    # should be 3
  - id: size_max
    type: u4
    doc: Maximum number of sectors in a given image file (capacity)
  - id: size_grain
    type: u4
    doc: Grain number of sectors
  - id: grain_dir
    type: u4
    doc: Grain directory sector number (usually 4)
  - id: num_grain_table_entries
    type: u4
    doc: Number of grain directory entries
  - id: next_free_grain
    type: u4
# rest fields are useless
