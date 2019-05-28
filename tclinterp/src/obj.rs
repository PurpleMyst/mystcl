use std::{ffi::CStr, os::raw::*, ptr::NonNull};

use pyo3::types::{PyAny, PyString, PyTuple};

use crate::{error::TclError, wrappers::Objv};

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

    pub fn empty() -> Result<Self, TclError> {
        NonNull::new(unsafe { tcl_sys::Tcl_NewObj() })
            .map(Self::new)
            .ok_or_else(|| TclError::new("Tcl_NewObj() returned NULL"))
    }

    pub fn as_ptr(&self) -> *mut tcl_sys::Tcl_Obj {
        self.ptr.as_ptr()
    }
}

// byte array
impl TclObj {
    pub fn as_bytes(&self) -> &[u8] {
        let mut length: i32 = Default::default();

        let data: *mut u8 = unsafe { tcl_sys::Tcl_GetByteArrayFromObj(self.as_ptr(), &mut length) };

        unsafe { std::slice::from_raw_parts(data as *const _, length as usize) }
    }
}

// Is `IntoTclObj` a better name for this?
pub trait ToTclObj {
    fn to_tcl_obj(self) -> TclObj;
}

impl ToTclObj for TclObj {
    fn to_tcl_obj(self) -> TclObj {
        self
    }
}

// XXX: This feels like a weird instance.
// It's basically meant for &&str
impl<T> ToTclObj for &T
where
    T: Copy + ToTclObj,
{
    fn to_tcl_obj(self) -> TclObj {
        self.clone().to_tcl_obj()
    }
}

impl ToTclObj for &[u8] {
    fn to_tcl_obj(self) -> TclObj {
        // `Tcl_NewStringObj` copies its argument.
        let ptr = unsafe { tcl_sys::Tcl_NewByteArrayObj(self.as_ptr(), self.len() as c_int) };
        TclObj::new(NonNull::new(ptr).unwrap())
    }
}

// FIXME: https://www.tcl.tk/man/tcl8.6/TclLib/Encoding.htm
impl ToTclObj for &str {
    fn to_tcl_obj(self) -> TclObj {
        let baites = self.as_bytes();
        let ptr = unsafe {
            tcl_sys::Tcl_NewStringObj(baites.as_ptr() as *const c_char, baites.len() as c_int)
        };
        TclObj::new(NonNull::new(ptr).unwrap())
    }
}

impl ToTclObj for &PyString {
    fn to_tcl_obj(self) -> TclObj {
        self.to_string_lossy().as_ref().to_tcl_obj()
    }
}

impl ToTclObj for &PyTuple {
    fn to_tcl_obj(self) -> TclObj {
        let objv = Objv::new(self);

        let ptr = unsafe { tcl_sys::Tcl_NewListObj(objv.len(), objv.as_ptr()) };
        let ptr = NonNull::new(ptr).unwrap();

        TclObj::new(ptr)
    }
}

impl ToTclObj for &PyAny {
    fn to_tcl_obj(self) -> TclObj {
        if let Ok(value) = self.downcast_ref::<PyString>() {
            value.to_tcl_obj()
        } else if let Ok(value) = self.downcast_ref::<PyTuple>() {
            value.to_tcl_obj()
        } else {
            unimplemented!("ToTclObj::to_tcl_obj for {:?}", self)
        }
    }
}

// FIXME: https://www.tcl.tk/man/tcl8.6/TclLib/Encoding.htm
impl std::fmt::Display for TclObj {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = unsafe { CStr::from_ptr(tcl_sys::Tcl_GetString(self.as_ptr())) };
        let s = s.to_str().unwrap();

        write!(f, "{}", s)
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
