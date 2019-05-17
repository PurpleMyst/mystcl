use std::os::raw::c_int;

use pyo3::{
    prelude::*,
    types::{PyAny, PyString},
};

use crate::tclinterp::TclInterp;

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

impl std::fmt::Debug for TclObjWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe {
            f.debug_struct("TclObjWrapper")
                .field("ptr", &format!("{:#p}", self.ptr))
                .field("refCount", &(*self.ptr).refCount)
                .field(
                    "str",
                    &std::ffi::CStr::from_ptr(tcl_sys::Tcl_GetString(self.ptr)),
                )
                .finish()
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

// XXX: Can we give this a better name?
pub struct TclPyTuple(Vec<TclObjWrapper>, Vec<*mut tcl_sys::Tcl_Obj>);

impl TclPyTuple {
    pub fn new<'a, I>(app: &mut TclInterp, it: I) -> PyResult<Self>
    where
        I: IntoIterator<Item = &'a PyAny>,
    {
        let wrappers = it
            .into_iter()
            .map(|arg| app.make_string_obj(arg))
            .collect::<Result<Vec<_>, _>>()?;

        let wrapper_ptrs = wrappers.iter().map(|arg| arg.ptr).collect::<Vec<_>>();

        // We keep both a vector of the wrappers themselves and the wrappers' pointers so we can
        // get a pointer to the wrappers' pointers and have it be valid as long as the wrappers
        // themselves are valid. Which is as long as the list itself is valid.
        Ok(Self(wrappers, wrapper_ptrs))
    }

    pub fn len(&self) -> c_int {
        // FIXME: Use into
        self.0.len() as c_int
    }

    pub fn as_ptr(&self) -> *const *mut tcl_sys::Tcl_Obj {
        self.1.as_ptr()
    }
}
