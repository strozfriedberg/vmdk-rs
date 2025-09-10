
#[derive(Debug, PartialEq, Eq)]
pub struct TestData<'a> {
    pub image_path: &'a str,
    pub image_size: u64,
    pub sha1: &'a str
}

pub const VMFS_THICK_000001: TestData = TestData {
     image_path: "data/vmfs_thick-000001.vmdk",
     image_size: 2097152,
     sha1: "2CCF34D146EF98204D1889FC44E94AD94E0B1CB6"
};

pub const VMFS_THICK: TestData = TestData {
     image_path: "data/vmfs_thick.vmdk",
     image_size: 2097152,
     sha1: "17EAF058191C5F2639D8F983CA7633E4F47087D1"
};

pub const TWO_GB_MAX_EXTENT_SPARSE: TestData = TestData {
    image_path: "data/twoGbMaxExtentSparse.vmdk",
    image_size: 10485760,
    sha1: "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
};

pub const TWO_GB_MAX_EXTENT_FLAT: TestData = TestData {
    image_path: "data/twoGbMaxExtentFlat.vmdk",
    image_size: 10485760,
    sha1: "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
};

pub const STREAM_OPTIMIZED: TestData = TestData {
    image_path: "data/streamOptimized.vmdk",
    image_size: 10485760,
    sha1: "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
};

pub const MONOLITHIC_SPARSE: TestData = TestData {
    image_path: "data/monolithicSparse.vmdk",
    image_size: 10485760,
    sha1: "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
};

pub const MONOLITHIC_FLAT: TestData = TestData {
    image_path: "data/monolithicFlat.vmdk",
    image_size: 10485760,
    sha1: "DD2FADE471D68658B2EBBFF7474F5D0A99DA8989"
};

pub const STREAM_OPTIMIZED_WITH_MARKERS: TestData = TestData {
    image_path: "data/streamOptimizedWithMarkers.vmdk",
    image_size: 1048576,
    sha1: "B6FD01DD1B93B3589E6D76F7507AF55C589EF69D"
};
