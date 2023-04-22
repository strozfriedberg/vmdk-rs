#![allow(dead_code)]
use std::{
    fs::File,
    io::{self, Read, SeekFrom},
    path::Path,
};

use memmap2::Mmap;

pub struct MMapper {
    file: String,
    mmap: Mmap,
    start: u64,
    end: u64,
}

impl MMapper {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path_str = path.as_ref().display().to_string();
        let file = File::open(&path).unwrap_or_else(|_| panic!("Could not open '{path_str}'"));
        let mmap = unsafe {
            Mmap::map(&file).unwrap_or_else(|_| panic!("Could not create mmap on '{path_str}'"))
        };
        let size = mmap.len();

        MMapper {
            file: path_str,
            mmap,
            start: 0,
            end: size as u64,
        }
    }

    pub fn align_to<T>(&self, offset: usize) -> &T {
        let (_, data, _) = unsafe { self.mmap[offset..].align_to::<T>() };
        &data[0]
    }

    pub fn set_range(&mut self, start: u64, end: u64) {
        self.start = start;
        self.end = end;
    }

    pub fn get_cursor(&self, start: usize, end: usize) -> io::Cursor<&[u8]> {
        io::Cursor::new(&self.mmap[start..end])
    }

    pub fn get_bytes(&self, offset: usize, size: usize) -> &[u8] {
        let s = &self.mmap[offset..offset + size];
        s
    }
}

impl Read for MMapper {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.start as usize;
        let len = buf.len();
        let e = n + len;

        if e < self.end as usize {
            let mut src = &self.mmap[n..e];
            let mut writer = buf;
            match io::copy(&mut src, &mut writer) {
                Ok(v) => Ok(v as usize),
                Err(e) => Err(e),
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "read {n}..{e} from '{}' which ends at {} (total {} bytes)",
                    self.file,
                    self.end,
                    self.mmap.len()
                )
                .as_str(),
            ))
        }
    }
}

impl io::Seek for MMapper {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => (self.end as i64 + n) as u64,
            SeekFrom::Current(n) => (self.start as i64 + n) as u64,
        };
        if new_pos < self.end {
            self.start = new_pos;
            Ok(0)
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, ""))
        }
    }
}
