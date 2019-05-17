use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_uint, c_void},
    ptr::NonNull,
    slice,
    sync::{Arc, Mutex},
};

use crate::{
    exceptions::TclError,
    tclinterp::TclInterp,
    wrappers::{TclObjWrapper, TclPyTuple},
};

use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
};

#[pyclass]
pub struct TkApp {
    interp: TclInterp,
}

impl TkApp {
    pub fn new() -> PyResult<Self> {
        Ok(Self {
            interp: TclInterp::new()?,
        })
    }
}

#[pymethods]
impl TkApp {
    #[args(args = "*")]
    fn call(&mut self, args: &PyTuple) -> PyResult<String> {
        // TODO: Put this in TclInterp
        let objv = TclPyTuple::new(&mut self.interp, args)?;

        self.interp.check_statuscode(unsafe {
            tcl_sys::Tcl_EvalObjv(self.interp.interp_ptr()?, objv.len(), objv.as_ptr(), 0)
        })?;

        self.interp.get_result()
    }

    fn delete(&mut self) -> PyResult<()> {
        self.interp.delete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pyo3::PyErrValue;

    #[test]
    fn test_new() {
        assert!(TkApp::new().is_ok());
    }

    macro_rules! pytuple {
        ($py:expr, [$($arg:expr),*]) => {
            &PyTuple::new($py, vec![$($arg),*]).as_ref($py)
        }
    }

    #[test]
    fn test_call() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = TkApp::new().expect("Could not create TkApp");

        assert_eq!(
            app.call(&pytuple!(py, ["return", "hello, world"])).unwrap(),
            "hello, world"
        );
    }

    fn errmsg(py: Python, err: &PyErr) -> String {
        match &err.pvalue {
            PyErrValue::ToObject(obj_candidate) => {
                obj_candidate.to_object(py).extract::<String>(py).unwrap()
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_delete() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = TkApp::new().expect("Could not create TkApp");
        app.delete().expect("Could not delete interpeter");

        if let Err(err) = app.call(&pytuple!(py, ["return", "test123"])) {
            assert_eq!(errmsg(py, &err), "Tried to use interpreter after deletion");
        } else {
            panic!("TkApp::call did not return Err(_) after TkApp::delete");
        }
    }
}
