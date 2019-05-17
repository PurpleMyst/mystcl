use std::{
    ffi::{CStr, CString},
    os::raw::{c_int, c_uint},
    ptr::NonNull,
};

use crate::{
    exceptions::TclError,
    wrappers::{TclObjWrapper, TclPyTuple},
};

use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
    PyErrValue,
};

#[pyclass]
pub struct TkApp {
    interp: Option<NonNull<tcl_sys::Tcl_Interp>>,
}

macro_rules! interp {
    ($interp:expr) => {
        $interp
            .interp
            .ok_or_else(|| TclError::py_err("Tried to use interpreter after deletion"))?
            .as_ptr()
    };
}

impl TkApp {
    pub fn new() -> PyResult<Self> {
        unsafe {
            let interp = Some(
                NonNull::new(tcl_sys::Tcl_CreateInterp())
                    .ok_or_else(|| TclError::py_err("Tcl_CreateInterp returned NULL"))?,
            );

            let mut inst = TkApp { interp };

            inst.check(tcl_sys::Tcl_Init(interp!(inst)))?;
            inst.check(tcl_sys::Tk_Init(interp!(inst)))?;

            // HACK: Closest thing we have to id(self)
            let id = &inst as *const _ as usize;
            let exit_var_name = format!("exit_var_{}", id);

            // NOTE: This is meant to be a literal {}
            inst.eval(String::from("rename exit {}"))?;
            inst.eval(format!("set {} false", exit_var_name))?;
            inst.eval(String::from("package require Tk"))?;
            inst.eval(format!("bind . <Destroy> {{ set {} true }}", exit_var_name))?;

            Ok(inst)
        }
    }

    fn eval(&mut self, code: String) -> PyResult<String> {
        // XXX: This string gets deleted on method exit. Does Tcl_Eval want its string to stay
        // around?
        let c_code = CString::new(code)?;

        self.check(unsafe { tcl_sys::Tcl_Eval(interp!(self), c_code.as_ptr()) })?;

        self.get_result()
    }

    fn get_result(&self) -> PyResult<String> {
        let result = unsafe { tcl_sys::Tcl_GetObjResult(interp!(self)) };

        if result.is_null() {
            Err(TclError::py_err("Tcl_GetObjResult returned NULL"))
        } else {
            Ok(unsafe { CStr::from_ptr(tcl_sys::Tcl_GetString(result)) }
                .to_str()?
                .to_owned())
        }
    }

    fn get_error(&self) -> PyResult<PyErr> {
        Ok(TclError::py_err(self.get_result()?))
    }

    fn check(&self, value: c_int) -> PyResult<()> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ => Err(self.get_error()?),
        }
    }

    pub(crate) fn make_string_obj(&mut self, arg: &PyAny) -> PyResult<TclObjWrapper> {
        let obj = if let Ok(s) = arg.downcast_ref::<PyString>() {
            TclObjWrapper::try_from_pystring(s)
        } else if let Ok(t) = arg.downcast_ref::<PyTuple>() {
            let objv = TclPyTuple::new(self, t)?;

            TclObjWrapper::new(unsafe { tcl_sys::Tcl_NewListObj(objv.len(), objv.as_ptr()) })
        } else {
            return Err(pyo3::exceptions::TypeError::py_err("Expected str or tuple"));
        };

        if let Some(obj) = obj {
            Ok(obj)
        } else {
            Err(self.get_error()?)
        }
    }
}

impl Drop for TkApp {
    fn drop(&mut self) {
        if self.interp.is_some() {
            self.delete().expect("Failed to drop TkApp");
        }
    }
}

#[pymethods]
impl TkApp {
    #[args(args = "*")]
    fn call(&mut self, args: &PyTuple) -> PyResult<String> {
        let objv = TclPyTuple::new(self, args)?;

        self.check(unsafe { tcl_sys::Tcl_EvalObjv(interp!(self), objv.len(), objv.as_ptr(), 0) })?;

        self.get_result()
    }

    fn delete(&mut self) -> PyResult<()> {
        unsafe { tcl_sys::Tcl_DeleteInterp(interp!(self)) };
        self.interp = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
