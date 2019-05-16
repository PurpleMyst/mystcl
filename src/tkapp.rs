use std::{
    ffi::{CStr, CString},
    os::raw::{c_int, c_uint},
};

use crate::{exceptions::TclError, wrappers::TclObjWrapper};

use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
};

#[pyclass]
pub struct TkApp {
    interp: *mut tcl_sys::Tcl_Interp,
}

impl TkApp {
    pub fn new() -> PyResult<Self> {
        unsafe {
            let interp = tcl_sys::Tcl_CreateInterp();

            if interp.is_null() {
                return Err(TclError::py_err("fuck"));
            };

            let mut inst = TkApp { interp };

            inst.check(tcl_sys::Tcl_Init(inst.interp))?;
            inst.check(tcl_sys::Tk_Init(inst.interp))?;

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
        unsafe {
            let c_code = CString::new(code)?.into_raw();
            self.check(tcl_sys::Tcl_Eval(self.interp, c_code))?;

            // XXX: Is this safe or does `Tcl_Eval` expect the string to stay around?
            let _c_code = CString::from_raw(c_code);
        }

        self.get_result()
    }

    fn get_result(&self) -> PyResult<String> {
        let result = unsafe { tcl_sys::Tcl_GetObjResult(self.interp) };

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

    fn make_string_obj(&self, arg: &PyAny) -> PyResult<TclObjWrapper> {
        let obj = if let Ok(s) = arg.downcast_ref::<PyString>() {
            TclObjWrapper::try_from_pystring(s)
        } else if let Ok(t) = arg.downcast_ref::<PyTuple>() {
            let objv_wrappers = t
                .into_iter()
                .map(|arg| self.make_string_obj(arg))
                .collect::<Result<Vec<_>, _>>()?;

            let objv = objv_wrappers.iter().map(|arg| arg.ptr).collect::<Vec<_>>();

            TclObjWrapper::new(unsafe {
                tcl_sys::Tcl_NewListObj(objv.len() as c_int, objv.as_ptr())
            })
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
        unsafe { tcl_sys::Tcl_DeleteInterp(self.interp) }
    }
}

#[pymethods]
impl TkApp {
    #[args(args = "*")]
    fn call(&mut self, args: &PyTuple) -> PyResult<String> {
        // We must keep these around even if we have the pointers themselves because the wrappers
        // manage the refcount.
        let objv_wrappers = args
            .into_iter()
            .map(|arg| self.make_string_obj(arg))
            .collect::<Result<Vec<_>, _>>()?;

        let objv = objv_wrappers.iter().map(|arg| arg.ptr).collect::<Vec<_>>();

        self.check(unsafe {
            tcl_sys::Tcl_EvalObjv(self.interp, objv.len() as c_int, objv.as_ptr(), 0)
        })?;

        self.get_result()
    }
}
