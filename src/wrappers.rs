use std::os::raw::c_int;

use pyo3::types::PyString;

pub struct TclObjWrapper {
    pub ptr: *mut tcl_sys::Tcl_Obj,
}

impl TclObjWrapper {
    pub fn new(ptr: *mut tcl_sys::Tcl_Obj) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            unsafe {
                (*ptr).refCount += 1;
            }
            Some(Self { ptr })
        }
    }

    pub fn try_from_pystring(s: &PyString) -> Option<Self> {
        let data = s.as_bytes();
        unsafe {
            Self::new(tcl_sys::Tcl_NewStringObj(
                data.as_ptr() as *const i8,
                data.len() as c_int,
            ))
        }
    }
}

impl Drop for TclObjWrapper {
    fn drop(&mut self) {
        unsafe {
            (*self.ptr).refCount -= 1;
        }
    }
}
