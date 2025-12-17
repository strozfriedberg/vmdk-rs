use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use kaitai::ReadSeek;
use std::{
    collections::HashMap,
    io::{Read, SeekFrom}
};

use crate::vmdk_reader::ReadError;

const SECTOR_SIZE: u64 = 512;

#[derive(Debug)]
pub struct SparseStorage {
    pub file: Box<dyn ReadSeek>,
    pub filename: String,
    pub grain_table: HashMap<u64 /*sector*/, u64 /*real sector in file*/>,
    // size size_grain * 512
    pub grain_size: u64,
    pub has_compressed_grain: bool,
    pub zeroed_grain_table_entry: bool
}

#[derive(Debug)]
pub struct FlatStorage {
    pub file: Box<dyn ReadSeek>,
    pub filename: String,
    pub offset: u64
}

#[derive(Debug)]
pub enum ExtentStorage {
    Sparse(SparseStorage),
    Flat(FlatStorage),
    Zero
}

impl ExtentStorage {
    pub fn read(
        &mut self,
        offset: u64,
        buf: &mut [u8]
    ) -> Result<usize, ReadError>
    {
        Ok(match self {
            &mut ExtentStorage::Sparse(ref mut storage) =>
                read_sparse(offset, storage, buf)?,
            &mut ExtentStorage::Flat(ref mut storage) => {
                let offset_in_extent = offset - storage.offset * SECTOR_SIZE;
                read_flat(offset_in_extent, storage, buf)?
            },
            ExtentStorage::Zero => read_zero(buf)
        })
    }
}

// We're going off the rails on a crazy grain
#[derive(Debug, thiserror::Error)]
#[error("Sanity check failed for grain index {0}")]
struct CrazyGrainIndex(u64);

#[derive(Debug)]
struct CompressedGrainHeader {
    _lba: u64,
    data_size: u32
}

fn read_and_decompress_grain(
    file: &mut Box<dyn ReadSeek>,
    grain_index: u64,
) -> std::io::Result<Vec<u8>>
{
    let cgh = CompressedGrainHeader {
        _lba: file.read_u64::<LittleEndian>()?,
        data_size: file.read_u32::<LittleEndian>()?,
    };

    let header: u16 = file.read_u16::<BigEndian>()?;

    // sanity check against expected zlib stream header values...
    if header % 31 != 0 || header & 0x0F00 != 8 << 8 || header & 0x0020 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            CrazyGrainIndex(grain_index)
        ));
    }

    let mut buffer = vec![0u8; cgh.data_size as usize];
    file.read_exact(buffer.as_mut_slice())?;

    let mut decoder = DeflateDecoder::new(&*buffer.as_mut_slice());
    let mut decoded_data = vec![];
    decoder.read_to_end(&mut decoded_data)?;

    Ok(decoded_data)
}

fn read_sparse(
    offset: u64,
    storage: &mut SparseStorage,
    mut buf: &mut [u8]
) -> Result<usize, ReadError>
{
    let grain_size = storage.grain_size * SECTOR_SIZE;
    let grain_index = offset / grain_size;
    let grain_data_offset = (offset % grain_size) as usize;

    let r = (grain_size as usize - grain_data_offset).min(buf.len());
    buf = &mut buf[..r];

    // NB: we know there is a grain for this index because we
    // registered it in the span map
    let sector_num = storage.grain_table.get(&grain_index)
        .expect("index must exist");

    if storage.zeroed_grain_table_entry && *sector_num == 1 {
        // handle zeroed GTE
        buf.fill(0);
    }
    else {
        let grain_start = *sector_num * SECTOR_SIZE;

        if storage.has_compressed_grain {
            storage.file.seek(SeekFrom::Start(grain_start))?;

            let grain_data = read_and_decompress_grain(
                &mut storage.file,
                grain_index
            )?;

            buf.clone_from_slice(
                &grain_data[grain_data_offset..grain_data_offset + r],
            );
        }
        else {
            storage.file.seek(SeekFrom::Start(grain_start + grain_data_offset as u64))?;
            storage.file.read_exact(buf)?;
        }
    }

    Ok(buf.len())
}

fn read_flat(
    local_offset: u64,
    storage: &mut FlatStorage,
    buf: &mut [u8]
) -> Result<usize, ReadError>
{
    // FLAT, VMFS
    let f = &mut storage.file;
    // NB: only ExtentKind::Flat has nonzero offset
    f.seek(SeekFrom::Start(local_offset + storage.offset))?;
    f.read_exact(buf)?;
    Ok(buf.len())
}

fn read_zero(
    buf: &mut [u8]
) -> usize
{
    buf.fill(0);
    buf.len()
}
