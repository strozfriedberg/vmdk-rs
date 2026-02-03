use kaitai::{BytesReader, KError, KStream, KStruct, ReadSeek};
use std::{
    io::{Read, Seek, SeekFrom},
    ops::Deref
};

use crate::errors::{DeserializationError, IoError, OpenErrorKind};
use crate::generated::{
    vmware_cowd::VmwareCowd,
    vmware_vmdk::{VmwareVmdk, VmwareVmdk_CompressionMethods}
};

#[derive(Debug)]
pub struct VmdkSparseFileHeader {
    pub src: Box<dyn ReadSeek>,
    pub size_max: u64,
    pub size_grain: u64,
    pub grain_dir: u64,
    pub num_grain_table_entries: u32,
    pub zeroed_grain_table_entry: bool,
    pub has_compressed_grain: bool,
    pub descriptor: String,
}

impl From<(&VmwareCowd, Box<dyn ReadSeek>)> for VmdkSparseFileHeader {
    fn from((h, src): (&VmwareCowd, Box<dyn ReadSeek>)) -> Self {
        Self {
            src,
            size_max: *h.size_max() as u64,
            size_grain: *h.size_grain() as u64,
            grain_dir: *h.grain_dir() as u64,
            num_grain_table_entries: *h.num_grain_table_entries(),
            zeroed_grain_table_entry: false,
            has_compressed_grain: false,
            descriptor: "".into(),
        }
    }
}

impl TryFrom<(&VmwareVmdk, Box<dyn ReadSeek>)> for VmdkSparseFileHeader {
    type Error = KError;

    fn try_from(
        (h, src): (&VmwareVmdk, Box<dyn ReadSeek>)
    ) -> Result<Self, Self::Error>
    {
        let descriptor = String::from_utf8_lossy(
            h.descriptor()?.deref()
        ).into();

        let grain_dir = if *h.flags().use_secondary_grain_dir() {
            *h.start_secondary_grain()
        }
        else {
            *h.start_primary_grain()
        } as u64;

        Ok(Self {
            src,
            size_max: *h.size_max() as u64,
            size_grain: *h.size_grain() as u64,
            grain_dir,
            num_grain_table_entries: *h.num_grain_table_entries() as u32,
            zeroed_grain_table_entry: *h.flags().zeroed_grain_table_entry(),
            has_compressed_grain: *h.flags().has_compressed_grain(),
            descriptor
        })
    }
}

fn try_vmware_cowd_header(
    io: BytesReader,
    src: Box<dyn ReadSeek>
) -> Result<VmdkSparseFileHeader, DeserializationError>
{
    VmwareCowd::read_into::<_, VmwareCowd>(&io, None, None)
        .map(|h| VmdkSparseFileHeader::from((&*h, src)))
        .map_err(|e| DeserializationError("VmwareCowd struct", e))
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

    Ok((&*h, src).try_into()?)
}

const COWD_SIGNATURE: [u8; 4] = [0x43, 0x4F, 0x57, 0x44];
const VMDK_SIGNATURE: [u8; 4] = [0x4B, 0x44, 0x4D, 0x56];

#[derive(Debug)]
pub enum FileType {
    Cowd,
    Vmdk
}

pub fn signature_to_file_type(sig: &[u8; 4]) -> Option<FileType> {
    match *sig {
        COWD_SIGNATURE => Some(FileType::Cowd),
        VMDK_SIGNATURE => Some(FileType::Vmdk),
        _ => None
    }
}

pub fn check_signature<T>(
    src: &mut T
) -> Result<Option<FileType>, std::io::Error>
where
    T: Read
{
    // check the signature
    let mut sig = [0; 4];
    src.read_exact(&mut sig)?;
    Ok(signature_to_file_type(&sig))
}

pub fn read_header<T: Read + Seek + Clone + 'static>(
    mut src: T,
) -> Result<VmdkSparseFileHeader, OpenErrorKind>
{
    src.seek(SeekFrom::Start(0))
        .map_err(IoError::from)?;

    let ft = check_signature(&mut src)
        .map_err(IoError::from)?;

    src.seek(SeekFrom::Start(0))
        .map_err(IoError::from)?;

    let rs = Box::new(src.clone()) as Box<dyn ReadSeek>;
    let io = BytesReader::try_from(rs)?;

    let src = Box::new(src) as Box<dyn ReadSeek>;

    match ft {
        Some(FileType::Cowd) => Ok(try_vmware_cowd_header(io, src)?),
        Some(FileType::Vmdk) => Ok(try_vmware_vmdk_header(io, src)?),
        None => Err(OpenErrorKind::InvalidFileHeader)
    }
}
