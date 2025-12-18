pub mod vmdk_reader;

#[cfg(feature = "capi")]
pub mod capi;

#[cfg(test)]
mod test_data;

#[cfg(test)]
mod test_helper;

mod bytessource;
mod cache;
mod cachereadseek;
mod descriptor;
mod dummycache;
mod errors;
mod extent_description;
mod extents;
mod filesource;
mod foyercache;
mod generated;
mod header;
mod placeholdersource;
mod s3source;
mod spans;
mod storage;

#[cfg(test)]
mod test {
    use crate::{
        test_data::*,
        test_helper::do_hash,
        vmdk_reader::VmdkReader
    };

    #[track_caller]
    fn assert_eq_test_data(exp: &TestData) {
        let mut reader = VmdkReader::open(exp.image_path).unwrap();
        let image_size = reader.image_size;

        let sha1 = do_hash(
            |offset, buf: &mut [u8]| {
                let buf_len = buf.len();
                reader.read_at_offset(offset, &mut buf[..buf_len])
                    .unwrap()
            },
            image_size,
            false
        );

        let act = TestData {
            image_path: exp.image_path,
            image_size: reader.image_size,
            sha1: &sha1
        };

        assert_eq!(&act, exp);
    }

    #[test]
    fn test_vmfs_thick_000001_vmdk() {
        assert_eq_test_data(&VMFS_THICK_000001);
    }

    #[test]
    fn test_vmfs_thick_vmdk() {
        assert_eq_test_data(&VMFS_THICK);
    }

    #[test]
    fn test_two_gb_max_extent_sparse_vmdk() {
        assert_eq_test_data(&TWO_GB_MAX_EXTENT_SPARSE);
    }

    #[test]
    fn test_two_gb_max_extent_flat_vmdk() {
        assert_eq_test_data(&TWO_GB_MAX_EXTENT_FLAT);
    }

    #[test]
    fn test_stream_optimized_vmdk() {
        assert_eq_test_data(&STREAM_OPTIMIZED);
    }

    #[test]
    fn test_monolithic_sparse_vmdk() {
        assert_eq_test_data(&MONOLITHIC_SPARSE);
    }

    #[test]
    fn test_monolithic_flat_vmdk() {
        assert_eq_test_data(&MONOLITHIC_FLAT);
    }

    #[test]
    fn test_stream_optimized_with_markers_vmdk() {
        // vmdk_dump.exe crashes on this stream optimized image with markers
        assert_eq_test_data(&STREAM_OPTIMIZED_WITH_MARKERS);
    }
}
