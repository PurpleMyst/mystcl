use std::{
    ffi::{CStr, CString},
    ops::{Deref, DerefMut},
    os::raw::{c_char, c_int, c_uint, c_void},
    ptr::NonNull,
    rc::Rc,
    sync::Mutex,
};

use crate::{
    exceptions::TclError,
    wrappers::{TclObjWrapper, TclPyTuple},
};

use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
};

struct TclInterpData {
    interp: Option<NonNull<tcl_sys::Tcl_Interp>>,
}

pub struct TclInterp(Rc<Mutex<TclInterpData>>);

impl TclInterp {
    pub fn new() -> PyResult<Self> {
        unsafe {
            let interp = Rc::new(Mutex::new(TclInterpData {
                interp: Some(
                    NonNull::new(tcl_sys::Tcl_CreateInterp())
                        .ok_or_else(|| TclError::py_err("Tcl_CreateInterp returned NULL"))?,
                ),
            }));

            let mut inst = Self(interp);

            inst.check_statuscode(tcl_sys::Tcl_Init(inst.interp_ptr()?))?;
            inst.check_statuscode(tcl_sys::Tk_Init(inst.interp_ptr()?))?;

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

    pub(crate) fn interp_ptr(&self) -> PyResult<*mut tcl_sys::Tcl_Interp> {
        self.0
            .lock()
            .unwrap()
            .interp
            .ok_or_else(|| TclError::py_err("Tried to use interpreter after deletion"))
            .map(|ptr| ptr.as_ptr())
    }

    fn eval(&mut self, code: String) -> PyResult<String> {
        let c_code = CString::new(code)?;

        self.check_statuscode(unsafe { tcl_sys::Tcl_Eval(self.interp_ptr()?, c_code.as_ptr()) })?;

        self.get_result()
    }

    fn get_string(&self, ptr: NonNull<tcl_sys::Tcl_Obj>) -> PyResult<String> {
        unsafe {
            Ok(CStr::from_ptr(tcl_sys::Tcl_GetString(ptr.as_ptr()))
                .to_str()?
                .to_owned())
        }
    }

    pub fn get_result(&self) -> PyResult<String> {
        NonNull::new(unsafe { tcl_sys::Tcl_GetObjResult(self.interp_ptr()?) })
            .ok_or_else(|| TclError::py_err("Tcl_GetObjResult returned NULL"))
            .and_then(|ptr| self.get_string(ptr))
    }

    pub fn get_error(&self) -> PyResult<PyErr> {
        Ok(TclError::py_err(self.get_result()?))
    }

    pub fn check_statuscode(&self, value: c_int) -> PyResult<()> {
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

    pub fn delete(&mut self) -> PyResult<()> {
        unsafe { tcl_sys::Tcl_DeleteInterp(self.interp_ptr()?) };
        self.0.lock().unwrap().interp = None;
        Ok(())
    }
}

impl Drop for TclInterp {
    fn drop(&mut self) {
        if self.0.lock().unwrap().interp.is_some() {
            self.delete().expect("Failed to drop TkApp");
        }
    }
}
