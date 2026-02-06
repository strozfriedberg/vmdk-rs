use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::DeflateDecoder;
use std::{
    collections::HashMap,
    io::{Read, SeekFrom}
};

use crate::{
    readseek::ReadSeek,
    vmdk_reader::ReadError
};

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
        match self {
            ExtentStorage::Sparse(storage) => storage.read(offset, buf),
            ExtentStorage::Flat(storage) => storage.read(offset, buf),
            ExtentStorage::Zero => Ok(read_zero(buf))
        }
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

impl SparseStorage {
    fn read(
        &mut self,
        offset: u64,
        mut buf: &mut [u8]
    ) -> Result<usize, ReadError>
    {
        let grain_size = self.grain_size * SECTOR_SIZE;
        let grain_index = offset / grain_size;
        let grain_data_offset = (offset % grain_size) as usize;

        let r = (grain_size as usize - grain_data_offset).min(buf.len());
        buf = &mut buf[..r];

        // NB: we know there is a grain for this index because we
        // registered it in the span map
        let sector_num = *self.grain_table.get(&grain_index)
            .expect("index must exist");

        if self.zeroed_grain_table_entry && sector_num == 1 {
            // handle zeroed GTE
            buf.fill(0);
        }
        else {
            let grain_start = sector_num * SECTOR_SIZE;

            if self.has_compressed_grain {
                self.file.seek(SeekFrom::Start(grain_start))?;

                let grain_data = read_and_decompress_grain(
                    &mut self.file,
                    grain_index
                )?;

                buf.clone_from_slice(
                    &grain_data[grain_data_offset..grain_data_offset + r],
                );
            }
            else {
                self.file.seek(SeekFrom::Start(grain_start + grain_data_offset as u64))?;
                self.file.read_exact(buf)?;
            }
        }

        Ok(buf.len())
    }
}

impl FlatStorage {
    fn read(
        &mut self,
        offset: u64,
        buf: &mut [u8]
    ) -> Result<usize, ReadError>
    {
        // FLAT, VMFS
        // NB: only ExtentKind::Flat may have nonzero extent offset
        self.file.seek(SeekFrom::Start(offset - self.offset * SECTOR_SIZE))?;
        self.file.read_exact(buf)?;
        Ok(buf.len())
    }
}

fn read_zero(buf: &mut [u8]) -> usize {
    buf.fill(0);
    buf.len()
}
