use libcommon_sys as sys;

pub struct Module(*mut sys::module_t);

impl Module {
    pub fn new(m: *mut sys::module_t) -> Self {
        unsafe {
            sys::module_require(m, std::ptr::null_mut());
        }
        Self(m)
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            sys::module_release(self.0);
        }
    }
}
