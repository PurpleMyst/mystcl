#![deny(unused_must_use)]

use std::{ffi::{CStr, CString}, os::raw::{c_int, c_uint}};

use pyo3::{prelude::*, wrap_pyfunction, create_exception};

#[pyclass]
struct TkApp {
    interp: *mut tcl_sys::Tcl_Interp,
}

create_exception!(mystcl, TclError, pyo3::exceptions::Exception);

impl TkApp {
    fn new() -> PyResult<Self> {
         unsafe {
            let interp = tcl_sys::Tcl_CreateInterp();

            if interp.is_null() { return Err(TclError::py_err("fuck")) };

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
             self.check( tcl_sys::Tcl_Eval(self.interp, c_code)  )?;

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
            Ok(unsafe { CStr::from_ptr(tcl_sys::Tcl_GetString(result)) }.to_str()?.to_owned())
        }
    }

    fn check(&self, value: c_int) -> PyResult<()> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ =>  Err(TclError::py_err(self.get_result()?))
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
}

#[pyfunction]
fn create() -> PyResult<Py<TkApp>> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let obj = Py::new(py, TkApp::new()?)?;
    Ok(obj)
}

#[pymodule]
fn mystcl(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(create))?;
    Ok(())
}
