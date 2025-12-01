use kaitai::{BytesReader, KStream, KStruct, ReadSeek};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    ops::Deref
};

use crate::errors::{DeserializationError, IoError, OpenError, OpenErrorKind};
use crate::generated::vmware_cowd::VmwareCowd;
use crate::generated::vmware_vmdk::{VmwareVmdk, VmwareVmdk_CompressionMethods};

#[derive(Debug)]
pub struct VmdkSparseFileHeader {
    pub io: Box<dyn ReadSeek>,
    pub size_max: u64,
    pub size_grain: u64,
    pub grain_dir: u64,
    pub num_grain_table_entries: u32,
    pub zeroed_grain_table_entry: bool,
    pub has_compressed_grain: bool,
    pub descriptor: String,
}

fn try_vmware_cowd_header(
    io: BytesReader,
    src: Box<dyn ReadSeek>
) -> Result<VmdkSparseFileHeader, DeserializationError>
{
    match VmwareCowd::read_into::<_, VmwareCowd>(&io, None, None) {
        Ok(h) => Ok(VmdkSparseFileHeader {
            io: src,
            size_max: *h.size_max() as u64,
            size_grain: *h.size_grain() as u64,
            grain_dir: *h.grain_dir() as u64,
            num_grain_table_entries: *h.num_grain_table_entries(),
            zeroed_grain_table_entry: false,
            has_compressed_grain: false,
            descriptor: "".into(),
        }),
        Err(e) => Err(DeserializationError("VmwareCowd struct", e))
    }
}

fn try_vmware_vmdk_header(
    io: BytesReader,
    src: Box<dyn ReadSeek>
) -> Result<VmdkSparseFileHeader, OpenErrorKind>
{
    let mut h = VmwareVmdk::read_into::<_, VmwareVmdk>(&io, None, None)
        .map_err(|e| DeserializationError("VmwareVmdk struct", e))?;

    if *h.start_primary_grain() == -1
        && *h.compression_method() == VmwareVmdk_CompressionMethods::Deflate
    {
        // If the grain directory sector number value is -1
        // (0xffffffffffffffff) (GD_AT_END) in a Stream-Optimized Compressed
        // Sparse Extent there should be a secondary file header stored at
        // offset -1024 relative from the end of the file (stream)
        io.seek(io.size() - 1024)
            .map_err(|e| DeserializationError("VmwareVmdk struct", e))?;

        h = VmwareVmdk::read_into::<_, VmwareVmdk>(&io, None, None)
            .map_err(|e| DeserializationError("VmwareVmdk struct", e))?;
    }

    let grain_dir = if *h.flags().use_secondary_grain_dir() {
        *h.start_secondary_grain() as u64
    }
    else {
        *h.start_primary_grain() as u64
    };

    let descriptor = String::from_utf8_lossy(h.descriptor()?.deref()).into();

    let hdr = VmdkSparseFileHeader {
        io: src,
        size_max: *h.size_max() as u64,
        size_grain: *h.size_grain() as u64,
        grain_dir,
        num_grain_table_entries: *h.num_grain_table_entries() as u32,
        zeroed_grain_table_entry: *h.flags().zeroed_grain_table_entry(),
        has_compressed_grain: *h.flags().has_compressed_grain(),
        descriptor
    };

    Ok(hdr)
}

pub fn open_header_impl<T: Read + Seek + 'static>(
    mut src_1: T,
    src_2: T
) -> Result<VmdkSparseFileHeader, OpenErrorKind>
{
    let mut first_bytes = [0; 4];

    src_1.read_exact(&mut first_bytes)
        .map_err(IoError::from)?;
//        .map_err(|e| IoError::SeekError(4, e))?;

    src_1.seek(SeekFrom::Start(0))
        .map_err(IoError::from)?;
//        .map_err(|e| IoError::SeekError(4, e))?;

    let rs = Box::new(src_2) as Box<dyn ReadSeek>;
    let io = BytesReader::try_from(rs)?;

    let src = Box::new(src_1) as Box<dyn ReadSeek>;

    match first_bytes.as_slice() {
        // COWD
        [0x43u8, 0x4Fu8, 0x57u8, 0x44u8] => Ok(try_vmware_cowd_header(io, src)?),
        // KDMV
        [0x4Bu8, 0x44u8, 0x4Du8, 0x56u8] => Ok(try_vmware_vmdk_header(io, src)?),
        _ => Err(OpenErrorKind::InvalidFileHeader)
    }
}

pub fn open_header<T: AsRef<Path>>(
    image_path: T
) -> Result<VmdkSparseFileHeader, OpenError>
{
    let mut src_1 = File::open(&image_path)
        .map_err(IoError::from)?;    

    let mut src_2 = File::open(&image_path)
        .map_err(IoError::from)?;    

    open_header_impl(src_1, src_2)
        .map_err(OpenError::from)
        .map_err(|e| e.with_path(&image_path))
}

pub fn read_descriptor_from_header<T: AsRef<Path>>(
    image_path: T
) -> Result<String, OpenError>
{
    open_header(image_path).map(|h| h.descriptor)
}
