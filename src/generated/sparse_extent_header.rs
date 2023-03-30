// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(irrefutable_let_patterns)]
#![allow(unused_comparisons)]
#![allow(arithmetic_overflow)]
#![allow(overflowing_literals)]

extern crate kaitai;
use kaitai::*;
use std::convert::{TryFrom, TryInto};
use std::cell::{Ref, Cell, RefCell};
use std::rc::{Rc, Weak};

/**
 * \sa https://www.virtualbox.org/browser/vbox/trunk/src/VBox/Storage/VMDK.cpp Source
 */

#[derive(Default, Debug, Clone)]
pub struct SparseExtentHeader {
    pub _root: SharedType<SparseExtentHeader>,
    pub _parent: SharedType<SparseExtentHeader>,
    pub _self: SharedType<Self>,
    magic_number: RefCell<Vec<u8>>,
    version: RefCell<u32>,
    flags: RefCell<u32>,
    capacity: RefCell<u64>,
    grain_size: RefCell<u64>,
    descriptor_offset: RefCell<u64>,
    descriptor_size: RefCell<u64>,
    num_gtes_per_gt: RefCell<u32>,
    rgd_offset: RefCell<u64>,
    gd_offset: RefCell<u64>,
    over_head: RefCell<u64>,
    unclean_shutdown: RefCell<bool>,
    single_end_line_char: RefCell<Vec<u8>>,
    non_end_line_char: RefCell<Vec<u8>>,
    double_end_line_char1: RefCell<Vec<u8>>,
    double_end_line_char2: RefCell<Vec<u8>>,
    compress_algorithm: RefCell<SparseExtentHeader_CompressionMethod>,
    pad: RefCell<Vec<u8>>,
    _io: RefCell<BytesReader>,
}
impl KStruct for SparseExtentHeader {
    type Root = SparseExtentHeader;
    type Parent = SparseExtentHeader;

    fn read<S: KStream>(
        self_rc: &OptRc<Self>,
        _io: &S,
        _root: SharedType<Self::Root>,
        _parent: SharedType<Self::Parent>,
    ) -> KResult<()> {
        *self_rc._io.borrow_mut() = _io.clone();
        self_rc._root.set(_root.get());
        self_rc._parent.set(_parent.get());
        self_rc._self.set(Ok(self_rc.clone()));
        let _rrc = self_rc._root.get_value().borrow().upgrade();
        let _prc = self_rc._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        *self_rc.magic_number.borrow_mut() = _io.read_bytes(4 as usize)?.into();
        if !(*self_rc.magic_number() == vec![0x4bu8, 0x44u8, 0x4du8, 0x56u8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0x4bu8, 0x44u8, 0x4du8, 0x56u8], *self_rc.magic_number(), _io, "/seq/0""#.to_string()));
        }
        *self_rc.version.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.flags.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.capacity.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.descriptor_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.descriptor_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.num_gtes_per_gt.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.rgd_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.gd_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.over_head.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.unclean_shutdown.borrow_mut() = _io.read_bits_int_be(1)? != 0;
        _io.align_to_byte()?;
        *self_rc.single_end_line_char.borrow_mut() = _io.read_bytes(1 as usize)?.into();
        if !(*self_rc.single_end_line_char() == vec![0xau8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0xau8], *self_rc.single_end_line_char(), _io, "/seq/12""#.to_string()));
        }
        *self_rc.non_end_line_char.borrow_mut() = _io.read_bytes(1 as usize)?.into();
        if !(*self_rc.non_end_line_char() == vec![0x20u8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0x20u8], *self_rc.non_end_line_char(), _io, "/seq/13""#.to_string()));
        }
        *self_rc.double_end_line_char1.borrow_mut() = _io.read_bytes(1 as usize)?.into();
        if !(*self_rc.double_end_line_char1() == vec![0xdu8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0xdu8], *self_rc.double_end_line_char1(), _io, "/seq/14""#.to_string()));
        }
        *self_rc.double_end_line_char2.borrow_mut() = _io.read_bytes(1 as usize)?.into();
        if !(*self_rc.double_end_line_char2() == vec![0xau8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0xau8], *self_rc.double_end_line_char2(), _io, "/seq/15""#.to_string()));
        }
        *self_rc.compress_algorithm.borrow_mut() = (_io.read_u2le()? as i64).try_into()?;
        *self_rc.pad.borrow_mut() = _io.read_bytes(433 as usize)?.into();
        Ok(())
    }
}
impl SparseExtentHeader {
}
impl SparseExtentHeader {
    pub fn magic_number(&self) -> Ref<Vec<u8>> {
        self.magic_number.borrow()
    }
}
impl SparseExtentHeader {
    pub fn version(&self) -> Ref<u32> {
        self.version.borrow()
    }
}
impl SparseExtentHeader {
    pub fn flags(&self) -> Ref<u32> {
        self.flags.borrow()
    }
}
impl SparseExtentHeader {
    pub fn capacity(&self) -> Ref<u64> {
        self.capacity.borrow()
    }
}
impl SparseExtentHeader {
    pub fn grain_size(&self) -> Ref<u64> {
        self.grain_size.borrow()
    }
}
impl SparseExtentHeader {
    pub fn descriptor_offset(&self) -> Ref<u64> {
        self.descriptor_offset.borrow()
    }
}
impl SparseExtentHeader {
    pub fn descriptor_size(&self) -> Ref<u64> {
        self.descriptor_size.borrow()
    }
}
impl SparseExtentHeader {
    pub fn num_gtes_per_gt(&self) -> Ref<u32> {
        self.num_gtes_per_gt.borrow()
    }
}
impl SparseExtentHeader {
    pub fn rgd_offset(&self) -> Ref<u64> {
        self.rgd_offset.borrow()
    }
}
impl SparseExtentHeader {
    pub fn gd_offset(&self) -> Ref<u64> {
        self.gd_offset.borrow()
    }
}
impl SparseExtentHeader {
    pub fn over_head(&self) -> Ref<u64> {
        self.over_head.borrow()
    }
}
impl SparseExtentHeader {
    pub fn unclean_shutdown(&self) -> Ref<bool> {
        self.unclean_shutdown.borrow()
    }
}
impl SparseExtentHeader {
    pub fn single_end_line_char(&self) -> Ref<Vec<u8>> {
        self.single_end_line_char.borrow()
    }
}
impl SparseExtentHeader {
    pub fn non_end_line_char(&self) -> Ref<Vec<u8>> {
        self.non_end_line_char.borrow()
    }
}
impl SparseExtentHeader {
    pub fn double_end_line_char1(&self) -> Ref<Vec<u8>> {
        self.double_end_line_char1.borrow()
    }
}
impl SparseExtentHeader {
    pub fn double_end_line_char2(&self) -> Ref<Vec<u8>> {
        self.double_end_line_char2.borrow()
    }
}
impl SparseExtentHeader {
    pub fn compress_algorithm(&self) -> Ref<SparseExtentHeader_CompressionMethod> {
        self.compress_algorithm.borrow()
    }
}
impl SparseExtentHeader {
    pub fn pad(&self) -> Ref<Vec<u8>> {
        self.pad.borrow()
    }
}
impl SparseExtentHeader {
    pub fn _io(&self) -> Ref<BytesReader> {
        self._io.borrow()
    }
}
#[derive(Debug, PartialEq, Clone)]
pub enum SparseExtentHeader_CompressionMethod {
    None,
    Deflate,
    Unknown(i64),
}

impl TryFrom<i64> for SparseExtentHeader_CompressionMethod {
    type Error = KError;
    fn try_from(flag: i64) -> KResult<SparseExtentHeader_CompressionMethod> {
        match flag {
            0 => Ok(SparseExtentHeader_CompressionMethod::None),
            1 => Ok(SparseExtentHeader_CompressionMethod::Deflate),
            _ => Ok(SparseExtentHeader_CompressionMethod::Unknown(flag)),
        }
    }
}

impl From<&SparseExtentHeader_CompressionMethod> for i64 {
    fn from(v: &SparseExtentHeader_CompressionMethod) -> Self {
        match *v {
            SparseExtentHeader_CompressionMethod::None => 0,
            SparseExtentHeader_CompressionMethod::Deflate => 1,
            SparseExtentHeader_CompressionMethod::Unknown(v) => v
        }
    }
}

impl Default for SparseExtentHeader_CompressionMethod {
    fn default() -> Self { SparseExtentHeader_CompressionMethod::Unknown(0) }
}

