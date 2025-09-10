use std::{
    ffi::{
        CStr,
        CString,
        c_char
    },
    path::Path,
    slice
};

use crate::vmdk_reader::VmdkReader;

#[repr(C)]
pub struct VmdkError {
    message: *mut c_char
}

impl Drop for VmdkError {
    fn drop(&mut self) {
        unsafe {
            if !self.message.is_null() {
                drop(Box::from_raw(self.message));
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmdk_free_error(err: *mut VmdkError) {
    if !err.is_null() {
        unsafe { drop(Box::from_raw(err)); }
    }
}

#[repr(C)]
pub struct VmdkHandle {
    reader: *mut VmdkReader,
    pub image_path: *const c_char,
    pub image_size: u64
}

fn path_to_cstring<'a, P>(path: P) -> Result<CString, String>
where
    P: AsRef<Path> + 'a
{
    path.as_ref()
        .to_str()
        .ok_or_else(|| format!("path is not UTF-8"))
        .and_then(|s| CString::new(s)
            .map_err(|_| format!("path contains an internal null"))
        )
}

impl VmdkHandle {
    fn new(reader: VmdkReader) -> Result<Self, String> {
        let image_path = path_to_cstring(&reader.image_path)?.into_raw();

        Ok(
            Self {
                image_path,
                image_size: reader.image_size,
                reader: Box::into_raw(Box::new(reader))
            }
        )
    }
}

impl Drop for VmdkHandle {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.reader) });
        drop(unsafe { CString::from_raw(self.image_path as *mut c_char) });
    }
}

fn fill_error<E: ToString>(e: E, err: *mut *mut VmdkError) {
    if !err.is_null() {
        // CString::new doesn't like internal nulls; the error message should
        // not have any, but we must deal with it nonetheless
        let message = CString::new(e.to_string())
            .unwrap_or_else(|_|
                CString::new(
                    format!(
                        "{}. Additionally, the original error message somehow contained an internal null, which should never happen.",
                        e.to_string().replace("\0", "\u{FFFD}")
                    )
                ).expect("inconceivable!")
            )
            .into_raw();

        unsafe { *err = Box::into_raw(Box::new(VmdkError { message })); }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmdk_open(
    image_path: *const c_char,
    err: *mut *mut VmdkError
) -> *mut VmdkHandle
{
    // convert path
    if image_path.is_null() {
       fill_error("image_path is null", err);
       return std::ptr::null_mut();
    }

    let p = unsafe { CStr::from_ptr(image_path) };

    let Ok(ip) = p.to_str() else {
        fill_error("image_path is not UTF-8", err);
        return std::ptr::null_mut();
    };

    // do the open
    match VmdkReader::open(ip) {
        Ok(reader) => match VmdkHandle::new(reader) {
            Ok(handle) => Box::into_raw(Box::new(handle)),
            Err(e) => {
                fill_error(e, err);
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            fill_error(e, err);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmdk_close(reader: *mut VmdkHandle) {
    if !reader.is_null() {
        drop(unsafe { Box::from_raw(reader) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vmdk_read(
    handle: *mut VmdkHandle,
    offset: u64,
    buf: *mut c_char,
    buflen: usize,
    err: *mut *mut VmdkError
) -> usize
{
    if handle.is_null() {
       fill_error("handle is null", err);
       return 0;
    }

    if buf.is_null() {
       fill_error("buf is null", err);
       return 0;
    }

    let buf = unsafe { slice::from_raw_parts_mut(buf as *mut u8, buflen) };
    unsafe { &*(*handle).reader }.read_at_offset(offset, buf)
        .unwrap_or_else(|e| { fill_error(e, err); 0 })
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{
        test_data::*,
        test_helper::do_hash
    };

    struct Holder<T> {
        ptr: *mut T
    }

    impl<T> Holder<T> {
        fn new(ptr: *mut T) -> Self {
            Self { ptr }
        }

        fn into_box(mut self) -> Box<T> {
            let ptr = self.ptr;
            self.ptr = std::ptr::null_mut();
            unsafe { Box::from_raw(ptr) }
        }
    }

    impl<T> Drop for Holder<T> {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { drop(Box::from_raw(self.ptr)) }
            }
        }
    }

    #[track_caller]
    fn assert_err(err: *mut VmdkError, message: &CStr) {
        assert!(!err.is_null());
        let err = unsafe { Box::from_raw(err) };

        assert!(!err.message.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(&*err.message) },
            message
        );
    }

    #[track_caller]
    fn assert_err_null(err: *mut VmdkError) {
        let err = Holder::new(err);
        assert!(err.ptr.is_null());
    }

    #[track_caller]
    fn assert_eq_test_data_no_hashing(handle: &VmdkHandle, exp: &TestData) {
        let image_path = unsafe { CStr::from_ptr(handle.image_path) }
            .to_str()
            .unwrap();

        let act = TestData {
            image_path,
            image_size: handle.image_size,
            sha1: exp.sha1
        };

        assert_eq!(&act, exp);
    }

    #[test]
    fn vmdk_open_null_path_null_err() {
        let h = Holder::new(unsafe {
            vmdk_open(
                std::ptr::null(),
                std::ptr::null_mut()
            )
        });

        assert!(h.ptr.is_null());
    }

    #[test]
    fn vmdk_open_nonexistent_path_null_err() {
        let path = c"bogus".as_ptr();

        let h = Holder::new(unsafe {
            vmdk_open(
                path,
                std::ptr::null_mut()
            )
        });

        assert!(h.ptr.is_null());
    }

    #[test]
    fn vmdk_open_null_paths() {
        let mut err = std::ptr::null_mut();

        let h = Holder::new(unsafe {
            vmdk_open(
                std::ptr::null(),
                &mut err
            )
        });

        assert_err(err, c"image_path is null");
        assert!(h.ptr.is_null());
    }

    #[test]
    fn vmkd_open_ok() {
        let path = c"data/vmfs_thick.vmdk".as_ptr();
        let mut err = std::ptr::null_mut();

        let h = Holder::new(unsafe {
            vmdk_open(
                path,
                &mut err
            )
        });

        assert_err_null(err);
        assert!(!h.ptr.is_null());

        let handle = h.into_box();
        assert_eq_test_data_no_hashing(&handle, &VMFS_THICK);

        let handle = Box::into_raw(handle);
        unsafe { vmdk_close(handle); }
    }

}
