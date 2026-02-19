use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

use crate::{
    errors::{DeserializationError, OpenErrorKind},
    readseek::ReadSeek
};

const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
struct Vmdk3Header {
    version: u32,
    flags: u32,
    disk_sectors: u32,
    granularity: u32,
    l1dir_offset: u32,
    l1dir_size: u32,
    file_sectors: u32,
    cylinders: u32,
    heads: u32,
    sectors_per_track: u32
}

impl Vmdk3Header {
    fn from_reader<R: Read>(r: &mut R) -> std::io::Result<Self> {
        Ok(
            Self {
                version: r.read_u32::<LittleEndian>()?,
                flags: r.read_u32::<LittleEndian>()?,
                disk_sectors: r.read_u32::<LittleEndian>()?,
                granularity: r.read_u32::<LittleEndian>()?,
                l1dir_offset: r.read_u32::<LittleEndian>()?,
                l1dir_size: r.read_u32::<LittleEndian>()?,
                file_sectors: r.read_u32::<LittleEndian>()?,
                cylinders: r.read_u32::<LittleEndian>()?,
                heads: r.read_u32::<LittleEndian>()?,
                sectors_per_track: r.read_u32::<LittleEndian>()?
            }
        )
    }
}

#[derive(Debug)]
pub struct Vmdk4Header {
    version: u32,
    flags: u32,
    capacity: u64,
    granularity: u64,
    pub desc_offset: u64,
    desc_size: u64,
    /* Number of GrainTableEntries per GrainTable */
    num_gtes_per_gt: u32,
    rgd_offset: u64,
    gd_offset: u64,
    grain_offset: u64,
    filler: u8,
    check_bytes: [u8; 4],
    compress_algorithm: u16
}

impl Vmdk4Header {
    fn from_reader_inner<R: Read>(r: &mut R) -> std::io::Result<Self> {
        Ok(
            Self {
                version: r.read_u32::<LittleEndian>()?,
                flags: r.read_u32::<LittleEndian>()?,
                capacity: r.read_u64::<LittleEndian>()?,
                granularity: r.read_u64::<LittleEndian>()?,
                desc_offset: r.read_u64::<LittleEndian>()?,
                desc_size: r.read_u64::<LittleEndian>()?,
                num_gtes_per_gt: r.read_u32::<LittleEndian>()?,
                rgd_offset: r.read_u64::<LittleEndian>()?,
                gd_offset: r.read_u64::<LittleEndian>()?,
                grain_offset: r.read_u64::<LittleEndian>()?,
                filler: r.read_u8()?,
                check_bytes: {
                    let mut cb = [0; 4];
                    r.read_exact(&mut cb)?;
                    cb
                },
                compress_algorithm: r.read_u16::<LittleEndian>()?
            }
        )
    }

    pub fn from_reader<R: Read + Seek>(
        r: &mut R
    ) -> std::io::Result<Self> {
        let h = Self::from_reader_inner(r)?;

        if h.gd_offset == 0xFFFFFFFFFFFFFFFF && h.compress_algorithm == 1 {
            // If the grain directory sector number value is -1
            // (0xFFFFFFFFFFFFFFFF) (GD_AT_END) in a Stream-Optimized Compressed
            // Sparse Extent there should be a secondary file header stored at
            // offset -1024 relative from the end of the file (stream)
            r.seek(SeekFrom::End(1024))?;
            Self::from_reader_inner(r)
        }
        else {
            Ok(h)
        }
    }
}

#[derive(Debug)]
struct VmdkSeSparseConstHeader {
    version: u64,
    capacity: u64,
    grain_size: u64,
    grain_table_size: u64,
    flags: u64,
    reserved1: u64,
    reserved2: u64,
    reserved3: u64,
    reserved4: u64,
    volatile_header_offset: u64,
    volatile_header_size: u64,
    journal_header_offset: u64,
    journal_header_size: u64,
    journal_offset: u64,
    journal_size: u64,
    grain_dir_offset: u64,
    grain_dir_size: u64,
    grain_tables_offset: u64,
    grain_tables_size: u64,
    free_bitmap_offset: u64,
    free_bitmap_size: u64,
    backmap_offset: u64,
    backmap_size: u64,
    grains_offset: u64,
    grains_size: u64
//    pad: [u8; 304]
}

impl VmdkSeSparseConstHeader {
    fn from_reader<R: Read>(r: &mut R) -> std::io::Result<Self> {
        Ok(
            Self {
                version: r.read_u64::<LittleEndian>()?,
                capacity: r.read_u64::<LittleEndian>()?,
                grain_size: r.read_u64::<LittleEndian>()?,
                grain_table_size: r.read_u64::<LittleEndian>()?,
                flags: r.read_u64::<LittleEndian>()?,
                reserved1: r.read_u64::<LittleEndian>()?,
                reserved2: r.read_u64::<LittleEndian>()?,
                reserved3: r.read_u64::<LittleEndian>()?,
                reserved4: r.read_u64::<LittleEndian>()?,
                volatile_header_offset: r.read_u64::<LittleEndian>()?,
                volatile_header_size: r.read_u64::<LittleEndian>()?,
                journal_header_offset: r.read_u64::<LittleEndian>()?,
                journal_header_size: r.read_u64::<LittleEndian>()?,
                journal_offset: r.read_u64::<LittleEndian>()?,
                journal_size: r.read_u64::<LittleEndian>()?,
                grain_dir_offset: r.read_u64::<LittleEndian>()?,
                grain_dir_size: r.read_u64::<LittleEndian>()?,
                grain_tables_offset: r.read_u64::<LittleEndian>()?,
                grain_tables_size: r.read_u64::<LittleEndian>()?,
                free_bitmap_offset: r.read_u64::<LittleEndian>()?,
                free_bitmap_size: r.read_u64::<LittleEndian>()?,
                backmap_offset: r.read_u64::<LittleEndian>()?,
                backmap_size: r.read_u64::<LittleEndian>()?,
                grains_offset: r.read_u64::<LittleEndian>()?,
                grains_size: r.read_u64::<LittleEndian>()?
            }
        )
    }
}

#[derive(Debug)]
pub struct VmdkSparseMeta {
    pub compressed: bool,
    pub has_zero_grain: bool,
    pub sectors: u64,
    pub l1_offset: u64,
    pub l1_len: u64,
    pub l2_len: u64,
    pub cluster_sectors: u64
}

impl From<Vmdk3Header> for VmdkSparseMeta {
    fn from(h: Vmdk3Header) -> Self {
        Self {
            compressed: false,
            has_zero_grain: false,
            sectors: h.disk_sectors as u64,
            l1_offset: h.l1dir_offset as u64 * SECTOR_SIZE,
            l1_len: h.l1dir_size as u64,
            l2_len: 4096,
            cluster_sectors: h.granularity as u64
        }
    }
}

impl From<Vmdk4Header> for VmdkSparseMeta {
    fn from(h: Vmdk4Header) -> Self {
        // check flags to select primary or secondary grain dir
        let l1_offset = if h.flags & 0x02 != 0 {
            h.rgd_offset
        }
        else {
            h.gd_offset
        } * SECTOR_SIZE;

        let l1_sectors_per_entry = (h.num_gtes_per_gt as u64) * h.granularity;
        let l1_len = h.capacity.div_ceil(l1_sectors_per_entry);

        Self {
            compressed: h.flags & 0x10000 != 0,
            has_zero_grain: h.flags & 0x04 != 0,
            sectors: h.capacity,
            l1_offset,
            l1_len,
            l2_len: h.num_gtes_per_gt as u64,
            cluster_sectors: h.granularity
        }
    }
}

#[derive(Debug)]
pub struct VmdkSeSparseMeta {
    pub sectors: u64,
    pub l1_offset: u64,
    pub l1_len: u64,
    pub cluster_sectors: u64
}

impl From<VmdkSeSparseConstHeader> for VmdkSeSparseMeta {
    fn from(h: VmdkSeSparseConstHeader) -> Self {
        Self {
            sectors: h.capacity,
            l1_offset: h.grain_dir_offset * SECTOR_SIZE,
            l1_len: h.grain_table_size / 8,
            cluster_sectors: h.grain_size
        }
    }
}

fn try_vmdk3_header(
    mut src: Box<dyn ReadSeek>
) -> Result<VmdkSparseMeta, DeserializationError>
{
    let h = Vmdk3Header::from_reader(&mut src)
        .map_err(|e| DeserializationError("Vmdk3Header", e))?;

    Ok(VmdkSparseMeta::from(h))
}

fn try_vmdk4_header(
    mut src: Box<dyn ReadSeek>
) -> Result<VmdkSparseMeta, OpenErrorKind>
{
    let h = Vmdk4Header::from_reader(&mut src)
        .map_err(|e| DeserializationError("Vmdk4Header", e))?;

    Ok(VmdkSparseMeta::from(h))
}

fn try_vmdk_sesparse_const_header(
    mut src: Box<dyn ReadSeek>
) -> Result<VmdkSeSparseMeta, OpenErrorKind>
{
    let h = VmdkSeSparseConstHeader::from_reader(&mut src)
        .map_err(|e| DeserializationError("VmdkSeSparseConstHeader", e))?;

    Ok(VmdkSeSparseMeta::from(h))
}

const VMDK3_MAGIC: [u8; 4] = [0x43, 0x4F, 0x57, 0x44];
const VMDK4_MAGIC: [u8; 4] = [0x4B, 0x44, 0x4D, 0x56];
const VMDK_SESPARSE_MAGIC: [u8; 8] = [0xBE, 0xBA, 0xFE, 0xCA, 0x00, 0x00, 0x00, 0x00];

#[derive(Debug, Eq, PartialEq)]
pub enum FileType {
    Vmdk3,
    Vmdk4,
    VmdkSeSparse
}

impl FileType {
    pub fn sig_len(&self) -> usize {
        match self {
            FileType::Vmdk3 | FileType::Vmdk4 => 4,
            FileType::VmdkSeSparse => 8
        }
    }
}

fn signature_to_file_type(sig: &[u8; 8]) -> Option<FileType> {
    match *sig {
        _ if sig.starts_with(&VMDK3_MAGIC) => Some(FileType::Vmdk3),
        _ if sig.starts_with(&VMDK4_MAGIC) => Some(FileType::Vmdk4),
        VMDK_SESPARSE_MAGIC => Some(FileType::VmdkSeSparse),
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
    let mut sig = [0; 8];
    src.read_exact(&mut sig)?;
    Ok(signature_to_file_type(&sig))
}

pub fn read_header_sparse<T: Read + Seek + Clone + 'static>(
    mut src: T
) -> Result<VmdkSparseMeta, OpenErrorKind>
{
    src.seek(SeekFrom::Start(0))?;

    let ft = check_signature(&mut src)?;

    if let Some(ft) = &ft {
        // return to end of signature
        src.seek(SeekFrom::Start(ft.sig_len() as u64))?;
    }

    let src = Box::new(src) as Box<dyn ReadSeek>;

    match ft {
        Some(FileType::Vmdk3) => Ok(try_vmdk3_header(src)?),
        Some(FileType::Vmdk4) => Ok(try_vmdk4_header(src)?),
        _ => Err(OpenErrorKind::InvalidFileHeader)
    }
}

pub fn read_header_sesparse<T: Read + Seek + Clone + 'static>(
    mut src: T
) -> Result<VmdkSeSparseMeta, OpenErrorKind>
{
    src.seek(SeekFrom::Start(0))?;

    let ft = check_signature(&mut src)?;

    if let Some(ft) = &ft {
        // return to end of signature
        src.seek(SeekFrom::Start(ft.sig_len() as u64))?;
    }

    if let Some(FileType::VmdkSeSparse) = ft {
        let src = Box::new(src) as Box<dyn ReadSeek>;
        Ok(try_vmdk_sesparse_const_header(src)?)
    }
    else {
        Err(OpenErrorKind::InvalidFileHeader)
    }
}
