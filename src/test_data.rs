
#[derive(Debug, PartialEq, Eq)]
pub struct TestData<'a> {
    pub image_path: &'a str,
    pub image_size: u64,
    pub sha1: &'a str
}

pub const VMFS_THICK_000001: TestData = TestData {
     image_path: "data/vmfs_thick-000001.vmdk",
     image_size: 2097152,
     sha1: "2ccf34d146ef98204d1889fc44e94ad94e0b1cb6"
};

pub const VMFS_THICK: TestData = TestData {
     image_path: "data/vmfs_thick.vmdk",
     image_size: 2097152,
     sha1: "17eaf058191c5f2639d8f983ca7633e4f47087d1"
};

pub const TWO_GB_MAX_EXTENT_SPARSE: TestData = TestData {
    image_path: "data/twoGbMaxExtentSparse.vmdk",
    image_size: 10485760,
    sha1: "dd2fade471d68658b2ebbff7474f5d0a99da8989"
};

pub const TWO_GB_MAX_EXTENT_FLAT: TestData = TestData {
    image_path: "data/twoGbMaxExtentFlat.vmdk",
    image_size: 10485760,
    sha1: "dd2fade471d68658b2ebbff7474f5d0a99da8989"
};

pub const STREAM_OPTIMIZED: TestData = TestData {
    image_path: "data/streamOptimized.vmdk",
    image_size: 10485760,
    sha1: "dd2fade471d68658b2ebbff7474f5d0a99da8989"
};

pub const MONOLITHIC_SPARSE: TestData = TestData {
    image_path: "data/monolithicSparse.vmdk",
    image_size: 10485760,
    sha1: "dd2fade471d68658b2ebbff7474f5d0a99da8989"
};

pub const MONOLITHIC_FLAT: TestData = TestData {
    image_path: "data/monolithicFlat.vmdk",
    image_size: 10485760,
    sha1: "dd2fade471d68658b2ebbff7474f5d0a99da8989"
};

pub const STREAM_OPTIMIZED_WITH_MARKERS: TestData = TestData {
    image_path: "data/streamOptimizedWithMarkers.vmdk",
    image_size: 1048576,
    sha1: "b6fd01dd1b93b3589e6d76f7507af55c589ef69d"
};
