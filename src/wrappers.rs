use std::os::raw::*;

use pyo3::{prelude::*, types::PyAny};

use crate::{tclinterp::TclInterp, tclobj::TclObj};

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

        let wrapper_ptrs = wrappers.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();

        // We keep both a vector of the wrappers themselves and the wrappers' pointers so we can
        // get a pointer to the wrappers' pointers and have it be valid as long as the wrappers
        // themselves are valid. Which is as long as the list itself is valid.
        Ok(Self(wrappers, wrapper_ptrs))
    }

    pub fn len(&self) -> c_int {
        debug_assert!(
            c_int::min_value() as usize > self.0.len()
                && self.0.len() < c_int::max_value() as usize
        );
        self.0.len() as c_int
    }

    pub fn as_ptr(&self) -> *const *mut tcl_sys::Tcl_Obj {
        self.1.as_ptr()
    }
}
