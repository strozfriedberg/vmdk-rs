mod errors;
mod extents;
mod generated;
mod header;
pub mod vmdk_reader;

#[cfg(test)]
mod test {
    use crate::vmdk_reader::VmdkReader;
    use sha1::{Digest, Sha1};

    #[track_caller]
    fn do_hash(vmdk_path: &str) -> String {
        let vmdk_reader = VmdkReader::open(vmdk_path).unwrap();

        let mut hasher = Sha1::new();
        let mut buf: Vec<u8> = vec![0; 1048576];
        let mut offset = 0;

        while offset < vmdk_reader.total_size() {
            let buf_size = buf.len();
            let read = vmdk_reader
                .read_at_offset(offset, &mut buf[..buf_size])
                .unwrap();

            if read == 0 {
                break;
            }

            hasher.update(&buf[..read]);

            offset += read as u64;
        }
        let result = hasher.finalize();
        format!("{:X}", result)
    }

    #[track_caller]
    fn assert_hash(
        image_path: &str,
        expected: &str
    ) {
        assert_eq!(do_hash(image_path), expected);
    }

    #[test]
    fn test_vmfs_thick_000001_vmdk() {
        assert_hash(
            "data/vmfs_thick-000001.vmdk",
            "2CCF34D146EF98204D1889FC44E94AD94E0B1CB6",
        );
    }

    #[test]
    fn test_vmfs_thick_vmdk() {
        assert_hash(
            "data/vmfs_thick.vmdk",
            "17EAF058191C5F2639D8F983CA7633E4F47087D1"
        );
    }

    #[test]
    fn test_two_gb_max_extent_sparse_vmdk() {
        assert_hash(
            "data/twoGbMaxExtentSparse.vmdk",
            "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
        );
    }

    #[test]
    fn test_two_gb_max_extent_flat_vmdk() {
        assert_hash(
            "data/twoGbMaxExtentFlat.vmdk",
            "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
        );
    }

    #[test]
    fn test_stream_optimized_vmdk() {
        assert_hash(
            "data/streamOptimized.vmdk",
            "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
        );
    }

    #[test]
    fn test_monolithic_sparse_vmdk() {
        assert_hash(
            "data/monolithicSparse.vmdk",
            "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
        );
    }

    #[test]
    fn test_monolithic_flat_vmdk() {
        assert_hash(
            "data/monolithicFlat.vmdk",
            "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
        );
    }

    #[test]
    fn test_stream_optimized_with_markers_vmdk() {
        // vmdk_dump.exe crashes on this stream optimized image with markers
        assert_hash(
            "data/streamOptimizedWithMarkers.vmdk",
            "B6FD01DD1B93B3589E6D76F7507AF55C589EF69D"
        );
    }
}
