use kaitai::ReadSeek;
use std::collections::HashMap;

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
