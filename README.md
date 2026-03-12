# vmdk-rs

`vmdk-rs` is a Rust library to read data from the VMware Virtual Disk (VMDK)
files. This project is in active development and is intended for forensic
research and testing.

### Supported extent file formats

* raw (flat)
* VMDK3 COWD (sparse)
* VMDK4 (sparse)
* SESPARSE

* flat types: monolithicFlat, 2GbMaxExtentFlat, vmfsThin
* sparse types: monolithicSparse, 2GbMaxExtentSparse, vmfsSparse, streamOptimized

### Supported format features

* grain compression
* data markers
* zeroed grain table entries
* delta links (snapshots)

### Usage example

Read from a VMDK in Rust:
```rust
    use vmdk::vmdk_reader::VmdkReader;

    let vmdk_reader = VmdkReader::open(&vmdk_path).unwrap();

    let mut buf: Vec<u8> = vec![0; 1048576];
    let mut offset = 0;
    while offset < vmdk_reader.total_size {
        let read = vmdk_reader.read_at_offset(offset, &mut buf).unwrap();
        if read == 0 {
            break;
        }

        // do something with buf[..read]

        offset += read as u64;
    }
```

Read from a VMDK in C:
```c
    #include "vmdkrs.h"

    VmdkError* err = nullptr;
    VmdkHandle* handle = vmdk_open(vmdk_path, &err);
    if (err) {
        eprintf(err->message);
        vmdk_free_error(err);
        return;
    }

    char buf[4096];
    uint64_t offset = 0;
    while (offset < handle.image_size) {
        uintptr_t r = vmdk_read(handle, offset, buf, sizeof(buf), &err);
        if (err) {
            eprintf(err->message);
            vmdk_free_error(err);
            return;
        }

        // do something with buf[..r]

        offset += r;
    }

    vmdk_close(handle);
```

### Comparison with other VMDK libraries

|                            | vmdk-rs             | libvmdk             | go-vmdk             |
| -------------------------- | ------------------- | ------------------- | ------------------- |
| VMDK3 (COWD)               | :check-mark-button: | :check-mark-button: |                     |
| VMDK4                      | :check-mark-button: |                     | :check-mark-button: |
| FLAT extents               | :check-mark-button: | :check-mark-button: | :check-mark-button: |
| VMFS extents               | :check-mark-button: | :check-mark-button: | :check-mark-button: |
| VMFSSPARSE extents         | :check-mark-button: | :check-mark-button: |                     |
| VMFSRAW extents            | :check-mark-button: | :check-mark-button: |                     |
| VMFSRDM extents            | :check-mark-button: | :check-mark-button: |                     |
| SPARSE extents             | :check-mark-button: | :check-mark-button: | :check-mark-button: |
| SESPARSE extents           | :check-mark-button: |                     |                     |
| ZERO extents               |                     | :check-mark-button: |                     |
| grain compression          | :check-mark-button: | :check-mark-button: |                     |
| data markers               | :check-mark-button: | :check-mark-button: |                     |
| zeroed grain table entries | :check-mark-button: | :check-mark-button: |                     |
} embedded descriptors       | :check-mark-button: | :check-mark-button: |                     |
| delta links (snapshots)    | :check-mark-button: | :check-mark-button: |                     |
| read from local filesystem | :check-mark-button: | :check-mark-button: | :check-mark-button: |
| read from S3               | :check-mark-button: |                     |                     |

### Copyright

Copyright 2025–6, LevelBlue. `vmdk-rs` is licensed under the Apache License, Version 2.0.
