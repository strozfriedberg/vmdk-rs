use std::{
    ffi::{
        CStr,
        CString,
        c_char
    },
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
    pub image_size: u64
}

impl VmdkHandle {
    fn new(reader: VmdkReader) -> Self {
        Self {
            image_size: reader.image_size,
            reader: Box::into_raw(Box::new(reader))
        }
    }
}

impl Drop for VmdkHandle {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.reader) });
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
        Ok(reader) => Box::into_raw(Box::new(VmdkHandle::new(reader))),
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
