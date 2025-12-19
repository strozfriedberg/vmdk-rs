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
        .ok_or_else(|| "path is not UTF-8".into())
        .and_then(|s| CString::new(s)
            .map_err(|_| "path contains an internal null".into())
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
    unsafe { &mut *(*handle).reader }.read_at_offset(offset, buf)
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
    fn assert_err_starts_with(err: *mut VmdkError, prefix: &CStr) {
        assert!(!err.is_null());
        let err = unsafe { Box::from_raw(err) };

        assert!(!err.message.is_null());

        let message = unsafe { CStr::from_ptr(&*err.message) };

        let msg_b = message.to_bytes();
        let pre_b = prefix.to_bytes();

        assert_eq!(
            &msg_b[..pre_b.len()],
            pre_b,
            "{message:?} does not start with {prefix:?}"
        );
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

    #[track_caller]
    fn assert_eq_test_data(h: *mut VmdkHandle, exp: &TestData) {
        let handle = unsafe { &*h };

        let sha1 = do_hash(
            |offset, buf: &mut [u8]| {
                let mut err = std::ptr::null_mut();
                let read = unsafe {
                    vmdk_read(
                        h,
                        offset,
                        buf.as_mut_ptr() as *mut c_char,
                        buf.len(),
                        &mut err
                    )
                };

                assert_err_null(err);
                read
            },
            handle.image_size,
            false
        );

        let image_path = unsafe { CStr::from_ptr(handle.image_path) }
            .to_str()
            .unwrap();

        let act = TestData {
            image_path,
            image_size: handle.image_size,
            sha1: &sha1
        };

        assert_eq!(&act, exp);
    }

    #[test]
    fn test_vmdk_open_null_path_null_err() {
        let h = Holder::new(unsafe {
            vmdk_open(
                std::ptr::null(),
                std::ptr::null_mut()
            )
        });

        assert!(h.ptr.is_null());
    }

    #[test]
    fn test_vmdk_open_nonexistent_path_null_err() {
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
    fn test_vmdk_open_null_paths() {
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
    fn test_vmkd_open_ok() {
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

    #[test]
    fn test_vmdk_close_null() {
        // nothing to test here other than that it doesn't crash
        unsafe { vmdk_close(std::ptr::null_mut()) };
    }

    #[test]
    fn test_vmdk_read_null_handle_null_err() {
        let mut buf: [c_char; 1] = [0];

        let r = unsafe {
            vmdk_read(
                std::ptr::null_mut(),
                0,
                buf.as_mut_ptr(),
                buf.len(),
                std::ptr::null_mut()
            )
        };

        assert_eq!(r, 0);
    }

    #[test]
    fn test_vmdk_read_null_buffer_null_err() {
        let path = c"data/vmfs_thick.vmdk".as_ptr();

        let h = Holder::new(unsafe {
            vmdk_open(
                path,
                std::ptr::null_mut()
            )
        });

        assert!(!h.ptr.is_null());

        let r = unsafe {
            vmdk_read(
                h.ptr,
                0,
                std::ptr::null_mut(),
                1,
                std::ptr::null_mut()
            )
        };

        assert_eq!(r, 0);
    }

    #[test]
    fn test_vmdk_read_null_handle() {
        let mut buf: [c_char; 1] = [0];
        let mut err = std::ptr::null_mut();

        let r = unsafe {
            vmdk_read(
                std::ptr::null_mut(),
                0,
                buf.as_mut_ptr(),
                buf.len(),
                &mut err
            )
        };

        assert_err(err, c"handle is null");
        assert_eq!(r, 0);
    }

    #[test]
    fn test_vmdk_read_null_buffer() {
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

        let r = unsafe {
            vmdk_read(
                h.ptr,
                0,
                std::ptr::null_mut(),
                1,
                &mut err
            )
        };

        assert_err(err, c"buf is null");
        assert_eq!(r, 0);
    }

    #[test]
    fn test_vmdk_read_offset_past_end() {
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

        let mut buf: [c_char; 1] = [0];

        let r = unsafe {
            vmdk_read(
                h.ptr,
                u64::MAX,
                buf.as_mut_ptr(),
                buf.len(),
                &mut err
            )
        };

        assert_err_starts_with(
            err,
            c"Requested offset 18446744073709551615 is beyond end of image"
        );
        assert_eq!(r, 0);
    }

    #[test]
    fn test_vmdk_read_and_hash() {
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

        let mut handle = h.into_box();
        assert_eq_test_data(&mut *handle, &VMFS_THICK);

        let handle = Box::into_raw(handle);
        unsafe { vmdk_close(handle); }
    }
}
