
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
