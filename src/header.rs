use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

use crate::{
    errors::{DeserializationError, OpenErrorKind},
    readseek::ReadSeek
};

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
struct Vmdk4Header {
    version: u32,
    flags: u32,
    capacity: u64,
    granularity: u64,
    desc_offset: u64,
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
    fn from_reader<R: Read>(r: &mut R) -> std::io::Result<Self> {
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
}

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

impl TryFrom<(&Vmdk3Header, Box<dyn ReadSeek>)> for VmdkSparseFileHeader {
    type Error = std::io::Error;

    fn try_from(
        (h, mut src): (&Vmdk3Header, Box<dyn ReadSeek>)
    ) -> Result<Self, Self::Error> {
        src.rewind()?;

        Ok(
            Self {
                src,
                size_max: h.disk_sectors as u64,
                size_grain: h.granularity as u64,
                grain_dir: h.l1dir_offset as u64,
                num_grain_table_entries: h.l1dir_size,
                zeroed_grain_table_entry: false,
                has_compressed_grain: false,
                descriptor: "".into(),
            }
        )
    }
}

impl TryFrom<(&Vmdk4Header, Box<dyn ReadSeek>)> for VmdkSparseFileHeader {
    type Error = std::io::Error;

    fn try_from(
        (h, mut src): (&Vmdk4Header, Box<dyn ReadSeek>)
    ) -> Result<Self, Self::Error>
    {
        let descriptor = if h.desc_offset > 0 {
            let mut buf = vec![0; 512 * 20];

// TODO: cleanup
            src.seek(SeekFrom::Start(h.desc_offset * 512))?;
            let mut p = 0;
            let end = loop {
                let r = src.read(&mut buf[p..])?;

                if r == 0 {
                    break p;
                }

                match buf[p..p + r].iter().position(|c| *c == 0x00) {
                    Some(i) => { break i; },
                    None => { p += r; }
                }
            };

            src.rewind()?;
            String::from_utf8_lossy(&buf[..end]).into()
        }
        else {
            "".into()
        };

        // check flags to select grain dir
        let grain_dir = if h.flags & 0x02 != 0 { h.rgd_offset } else { h.gd_offset };

        let zeroed_grain_table_entry = h.flags & 0x04 != 0;
        let has_compressed_grain = h.flags & 0x10000 != 0;

        Ok(Self {
            src,
            size_max: h.capacity,
            size_grain: h.granularity,
            grain_dir,
            num_grain_table_entries: h.num_gtes_per_gt,
            zeroed_grain_table_entry,
            has_compressed_grain,
            descriptor
        })
    }
}

fn try_cowd_header(
    mut src: Box<dyn ReadSeek>
) -> Result<VmdkSparseFileHeader, DeserializationError>
{
    let h = Vmdk3Header::from_reader(&mut src)
        .map_err(|e| DeserializationError("Vmdk3Header struct", e))?;

    Ok(
        VmdkSparseFileHeader::try_from((&h, src))
            .map_err(|e| DeserializationError("Vmdk3Header struct", e))?
    )
}

fn try_vmdk_header(
    mut src: Box<dyn ReadSeek>
) -> Result<VmdkSparseFileHeader, OpenErrorKind>
{
    let mut h = Vmdk4Header::from_reader(&mut src)
        .map_err(|e| DeserializationError("Vmdk4Header struct", e))?;

    if h.gd_offset == 0xFFFFFFFFFFFFFFFF && h.compress_algorithm == 1 {
        // If the grain directory sector number value is -1
        // (0xffffffffffffffff) (GD_AT_END) in a Stream-Optimized Compressed
        // Sparse Extent there should be a secondary file header stored at
        // offset -1024 relative from the end of the file (stream)

        src.seek(SeekFrom::End(1024))
            .map_err(|e| DeserializationError("Vmdk4Header struct", e))?;

        h = Vmdk4Header::from_reader(&mut src)
            .map_err(|e| DeserializationError("Vmdk4Header struct", e))?;
    }

    src.rewind()?;

    Ok(
        VmdkSparseFileHeader::try_from((&h, src))
            .map_err(|e| DeserializationError("Vmdk4Header struct", e))?
    )
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
    src.seek(SeekFrom::Start(0))?;

    let ft = check_signature(&mut src)?;

    let src = Box::new(src) as Box<dyn ReadSeek>;

    match ft {
        Some(FileType::Cowd) => Ok(try_cowd_header(src)?),
        Some(FileType::Vmdk) => Ok(try_vmdk_header(src)?),
        None => Err(OpenErrorKind::InvalidFileHeader)
    }
}
