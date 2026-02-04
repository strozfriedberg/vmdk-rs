// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

#[allow(unused_imports)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(irrefutable_let_patterns)]
#[allow(unused_comparisons)]

extern crate kaitai;
use kaitai::*;
use std::convert::{TryFrom, TryInto};
use std::cell::{Ref, Cell, RefCell};
use std::rc::{Rc, Weak};

/**
 * \sa https://lists.nongnu.org/archive/html/qemu-block/2019-06/msg00932.html Source
 */

#[derive(Default, Debug, Clone)]
pub struct VmwareSesparse {
    pub _root: SharedType<VmwareSesparse>,
    pub _parent: SharedType<VmwareSesparse>,
    pub _self: SharedType<Self>,
    magic: RefCell<u64>,
    version: RefCell<u64>,
    capacity: RefCell<u64>,
    grain_size: RefCell<u64>,
    grain_table_size: RefCell<u64>,
    flags: RefCell<u64>,
    reserved1: RefCell<u64>,
    reserved2: RefCell<u64>,
    reserved3: RefCell<u64>,
    reserved4: RefCell<u64>,
    volatile_header_offset: RefCell<u64>,
    volatile_header_size: RefCell<u64>,
    journal_header_offset: RefCell<u64>,
    journal_header_size: RefCell<u64>,
    journal_offset: RefCell<u64>,
    journal_size: RefCell<u64>,
    grain_dir_offset: RefCell<u64>,
    grain_dir_size: RefCell<u64>,
    grain_tables_offset: RefCell<u64>,
    grain_tables_size: RefCell<u64>,
    free_bitmap_offset: RefCell<u64>,
    free_bitmap_size: RefCell<u64>,
    backmap_offset: RefCell<u64>,
    backmap_size: RefCell<u64>,
    grains_offset: RefCell<u64>,
    grains_size: RefCell<u64>,
    _io: RefCell<BytesReader>,
}
impl KStruct for VmwareSesparse {
    type Root = VmwareSesparse;
    type Parent = VmwareSesparse;

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
        *self_rc.magic.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.version.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.capacity.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_table_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.flags.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.reserved1.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.reserved2.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.reserved3.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.reserved4.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.volatile_header_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.volatile_header_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.journal_header_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.journal_header_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.journal_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.journal_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_dir_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_dir_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_tables_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grain_tables_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.free_bitmap_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.free_bitmap_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.backmap_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.backmap_size.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grains_offset.borrow_mut() = _io.read_u8le()?.into();
        *self_rc.grains_size.borrow_mut() = _io.read_u8le()?.into();
        Ok(())
    }
}
impl VmwareSesparse {
}
impl VmwareSesparse {
    pub fn magic(&self) -> Ref<'_, u64> {
        self.magic.borrow()
    }
}
impl VmwareSesparse {
    pub fn version(&self) -> Ref<'_, u64> {
        self.version.borrow()
    }
}
impl VmwareSesparse {
    pub fn capacity(&self) -> Ref<'_, u64> {
        self.capacity.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_size(&self) -> Ref<'_, u64> {
        self.grain_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_table_size(&self) -> Ref<'_, u64> {
        self.grain_table_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn flags(&self) -> Ref<'_, u64> {
        self.flags.borrow()
    }
}
impl VmwareSesparse {
    pub fn reserved1(&self) -> Ref<'_, u64> {
        self.reserved1.borrow()
    }
}
impl VmwareSesparse {
    pub fn reserved2(&self) -> Ref<'_, u64> {
        self.reserved2.borrow()
    }
}
impl VmwareSesparse {
    pub fn reserved3(&self) -> Ref<'_, u64> {
        self.reserved3.borrow()
    }
}
impl VmwareSesparse {
    pub fn reserved4(&self) -> Ref<'_, u64> {
        self.reserved4.borrow()
    }
}
impl VmwareSesparse {
    pub fn volatile_header_offset(&self) -> Ref<'_, u64> {
        self.volatile_header_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn volatile_header_size(&self) -> Ref<'_, u64> {
        self.volatile_header_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn journal_header_offset(&self) -> Ref<'_, u64> {
        self.journal_header_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn journal_header_size(&self) -> Ref<'_, u64> {
        self.journal_header_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn journal_offset(&self) -> Ref<'_, u64> {
        self.journal_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn journal_size(&self) -> Ref<'_, u64> {
        self.journal_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_dir_offset(&self) -> Ref<'_, u64> {
        self.grain_dir_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_dir_size(&self) -> Ref<'_, u64> {
        self.grain_dir_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_tables_offset(&self) -> Ref<'_, u64> {
        self.grain_tables_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn grain_tables_size(&self) -> Ref<'_, u64> {
        self.grain_tables_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn free_bitmap_offset(&self) -> Ref<'_, u64> {
        self.free_bitmap_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn free_bitmap_size(&self) -> Ref<'_, u64> {
        self.free_bitmap_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn backmap_offset(&self) -> Ref<'_, u64> {
        self.backmap_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn backmap_size(&self) -> Ref<'_, u64> {
        self.backmap_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn grains_offset(&self) -> Ref<'_, u64> {
        self.grains_offset.borrow()
    }
}
impl VmwareSesparse {
    pub fn grains_size(&self) -> Ref<'_, u64> {
        self.grains_size.borrow()
    }
}
impl VmwareSesparse {
    pub fn _io(&self) -> Ref<'_, BytesReader> {
        self._io.borrow()
    }
}
