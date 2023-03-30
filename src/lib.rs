
#![allow(unused)]
extern crate kaitai;
use self::kaitai::*;

pub mod generated;
use generated::sparse_extent_header::*;
use generated::vmware_vmdk::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let _io = BytesReader::open("C:/temp/VM/Linux Lite 5.8 (64bit).vmdk").unwrap();
        let res: KResult<OptRc<SparseExtentHeader>> = SparseExtentHeader::read_into(&_io, None, None);
        let r : OptRc<SparseExtentHeader>;

        if let Err(err) = res {
            panic!("{:?}", err);
        } else {
            r = res.unwrap();
        }

        println!("{:?}", r);
    }

}
