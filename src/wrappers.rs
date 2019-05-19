use std::os::raw::*;

use pyo3::{
    prelude::*,
    types::{PyAny, PyString},
};

use crate::tclinterp::TclInterp;

pub struct TclObj {
    pub ptr: *mut tcl_sys::Tcl_Obj,
}

impl TclObj {
    pub fn new(ptr: *mut tcl_sys::Tcl_Obj) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            unsafe {
                (*ptr).refCount += 1;
            }
            Some(TclObj { ptr })
        }
    }

    fn try_from_bytes(b: &[u8]) -> Option<Self> {
        unsafe {
            Self::new(tcl_sys::Tcl_NewStringObj(
                b.as_ptr() as *const c_char,
                b.len() as c_int,
            ))
        }
    }

    pub fn try_from_string(s: String) -> Option<Self> {
        Self::try_from_bytes(s.as_bytes())
    }

    pub fn try_from_pystring(s: &PyString) -> Option<Self> {
        Self::try_from_bytes(s.as_bytes())
    }
}

impl std::fmt::Debug for TclObj {
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

impl Drop for TclObj {
    fn drop(&mut self) {
        unsafe {
            (*self.ptr).refCount -= 1;
        }
    }
}

pub struct Objv(Vec<TclObj>, Vec<*mut tcl_sys::Tcl_Obj>);

impl Objv {
    pub fn new<'a, I>(app: &TclInterp, it: I) -> PyResult<Self>
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
