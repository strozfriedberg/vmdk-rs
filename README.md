# vmdk-rs 

`vmdk-rs` is a Rust library to read data from the VMware Virtual Disk (VMDK) files.
This project is in active development and is intended for forensic research and testing.

### Read supported extent file formats:
* RAW (flat)
* COWD (sparse)
* VMDK (sparse)

### We support file formats:
* flat (monolithicFlat, 2GbMaxExtentFlat, vmfsThin)
* sparse (monolithicSparse, 2GbMaxExtentSparse, vmfsSparse, streamOptimized)

### Supported VMDK format features:
* grain compression
* data markers
* delta links (snapshots)

Sample of usage:
```
    use vmdk::vmdk_reader::VmdkReader;

    fn read_vmdk(vmdk_path: &str) {
        let vmdk_reader = VmdkReader::open(&vmdk_path).unwrap();

        let mut buf: Vec<u8> = vec![0; 1048576];
        let mut offset = 0;
        while offset < vmdk_reader.total_size {
            let readed = vmdk_reader.read_at_offset(offset, &mut buf).unwrap();
            if readed == 0 {
                break;
            }

            // process buf[..readed]

            offset += readed as u64;
        }
    }

```

### Copyright
Copyright 2025, Aon. `vmdk-rs` is licensed under the Apache License, Version 2.0.
