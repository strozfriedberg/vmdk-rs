use std::io::{Read, Seek};

pub trait ReadSeek: Read + Seek {}

impl<T> ReadSeek for T where T: Read + Seek {}

impl std::fmt::Debug for dyn ReadSeek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadSeek")
    }
}
