use std::{ffi::CStr, os::raw::*, ptr::NonNull};

pub struct TclObj {
    ptr: NonNull<tcl_sys::Tcl_Obj>,
}

impl TclObj {
    pub fn new(ptr: NonNull<tcl_sys::Tcl_Obj>) -> Self {
        unsafe {
            (*ptr.as_ptr()).refCount += 1;
        }
        TclObj { ptr }
    }

    pub fn as_ptr(&self) -> *mut tcl_sys::Tcl_Obj {
        self.ptr.as_ptr()
    }
}

impl<T> From<T> for TclObj
where
    T: AsRef<[u8]>,
{
    fn from(s: T) -> Self {
        let s = s.as_ref();

        // We do not care about the lifetime of `s` due to `Tcl_NewStringObj` creating a copy of
        // its argument.
        let ptr: *mut _ =
            unsafe { tcl_sys::Tcl_NewStringObj(s.as_ptr() as *const c_char, s.len() as c_int) };
        Self::new(NonNull::new(ptr).unwrap())
    }
}

impl std::fmt::Display for TclObj {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = unsafe { CStr::from_ptr(tcl_sys::Tcl_GetString(self.as_ptr())) };

        write!(f, "{}", s.to_str().unwrap())
    }
}

impl std::fmt::Debug for TclObj {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe {
            let ptr = self.as_ptr();

            f.debug_struct("TclObjWrapper")
                .field("ptr", &format!("{:#p}", ptr))
                .field("refCount", &(*ptr).refCount)
                .field(
                    "str",
                    &std::ffi::CStr::from_ptr(tcl_sys::Tcl_GetString(ptr)),
                )
                .finish()
        }
    }
}

impl Drop for TclObj {
    fn drop(&mut self) {
        unsafe {
            (*self.as_ptr()).refCount -= 1;
        }
    }
}
