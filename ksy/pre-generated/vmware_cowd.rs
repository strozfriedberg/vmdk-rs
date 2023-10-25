// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

#[allow(unused_imports)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(irrefutable_let_patterns)]
#[allow(unused_comparisons)]
#[allow(arithmetic_overflow)]
#[allow(overflowing_literals)]

extern crate kaitai;
use kaitai::*;
use std::convert::{TryFrom, TryInto};
use std::cell::{Ref, Cell, RefCell};
use std::rc::{Rc, Weak};

/**
 * \sa https://github.com/libyal/libvmdk/blob/main/documentation/VMWare%20Virtual%20Disk%20Format%20(VMDK).asciidoc#51-file-header Source
 */

#[derive(Default, Debug, Clone)]
pub struct VmwareCowd {
    pub _root: SharedType<VmwareCowd>,
    pub _parent: SharedType<VmwareCowd>,
    pub _self: SharedType<Self>,
    magic: RefCell<Vec<u8>>,
    version: RefCell<u32>,
    flags: RefCell<u32>,
    size_max: RefCell<u32>,
    size_grain: RefCell<u32>,
    grain_dir: RefCell<u32>,
    num_grain_table_entries: RefCell<u32>,
    next_free_grain: RefCell<u32>,
    _io: RefCell<BytesReader>,
}
impl KStruct for VmwareCowd {
    type Root = VmwareCowd;
    type Parent = VmwareCowd;

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
        *self_rc.magic.borrow_mut() = _io.read_bytes(4 as usize)?.into();
        if !(*self_rc.magic() == vec![0x43u8, 0x4fu8, 0x57u8, 0x44u8]) {
            return Err(KError::ValidationNotEqual(r#"vec![0x43u8, 0x4fu8, 0x57u8, 0x44u8], *self_rc.magic(), _io, "/seq/0""#.to_string()));
        }
        *self_rc.version.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.flags.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.size_max.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.size_grain.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.grain_dir.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.num_grain_table_entries.borrow_mut() = _io.read_u4le()?.into();
        *self_rc.next_free_grain.borrow_mut() = _io.read_u4le()?.into();
        Ok(())
    }
}
impl VmwareCowd {
}
impl VmwareCowd {
    pub fn magic(&self) -> Ref<Vec<u8>> {
        self.magic.borrow()
    }
}
impl VmwareCowd {
    pub fn version(&self) -> Ref<u32> {
        self.version.borrow()
    }
}
impl VmwareCowd {
    pub fn flags(&self) -> Ref<u32> {
        self.flags.borrow()
    }
}

/**
 * Maximum number of sectors in a given image file (capacity)
 */
impl VmwareCowd {
    pub fn size_max(&self) -> Ref<u32> {
        self.size_max.borrow()
    }
}

/**
 * Grain number of sectors
 */
impl VmwareCowd {
    pub fn size_grain(&self) -> Ref<u32> {
        self.size_grain.borrow()
    }
}

/**
 * Grain directory sector number (usually 4)
 */
impl VmwareCowd {
    pub fn grain_dir(&self) -> Ref<u32> {
        self.grain_dir.borrow()
    }
}

/**
 * Number of grain directory entries
 */
impl VmwareCowd {
    pub fn num_grain_table_entries(&self) -> Ref<u32> {
        self.num_grain_table_entries.borrow()
    }
}
impl VmwareCowd {
    pub fn next_free_grain(&self) -> Ref<u32> {
        self.next_free_grain.borrow()
    }
}
impl VmwareCowd {
    pub fn _io(&self) -> Ref<BytesReader> {
        self._io.borrow()
    }
}