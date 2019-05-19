use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::*,
    ptr::NonNull,
    rc::Rc,
    slice,
    sync::Mutex,
};

use crate::{exceptions::TclError, tclobj::TclObj, wrappers::Objv};

use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
};

mod createcommand;
use createcommand::*;

struct TclInterpData {
    interp: Option<NonNull<tcl_sys::Tcl_Interp>>,
    commands: HashMap<CString, *mut CommandData>,
}

#[derive(Clone)]
pub struct TclInterp(Rc<Mutex<TclInterpData>>);

impl TclInterp {
    pub fn new() -> PyResult<Self> {
        unsafe {
            let interp = Rc::new(Mutex::new(TclInterpData {
                interp: Some(
                    NonNull::new(tcl_sys::Tcl_CreateInterp())
                        .ok_or_else(|| TclError::py_err("Tcl_CreateInterp returned NULL"))?,
                ),
                commands: Default::default(),
            }));

            let inst = Self(interp);

            inst.check_statuscode(tcl_sys::Tcl_Init(inst.interp_ptr()?))?;

            Ok(inst)
        }
    }

    pub fn init_tk(&mut self) -> PyResult<()> {
        self.check_statuscode(unsafe { tcl_sys::Tk_Init(self.interp_ptr()?) })?;

        let id = &self as *const _ as usize;
        let exit_var_name = format!("exit_var_{}", id);
        self.eval(String::from("package require Tk"))?;
        self.eval(String::from("rename exit {}"))?;
        self.eval(format!("set {} false", exit_var_name))?;
        self.eval(format!("bind . <Destroy> {{ set {} true }}", exit_var_name))?;

        Ok(())
    }

    pub(crate) fn interp_ptr(&self) -> PyResult<*mut tcl_sys::Tcl_Interp> {
        self.0
            .lock()
            .unwrap()
            .interp
            .ok_or_else(|| TclError::py_err("Tried to use interpreter after deletion"))
            .map(|ptr| ptr.as_ptr())
    }

    pub fn eval(&mut self, code: String) -> PyResult<String> {
        let c_code = CString::new(code)?;

        self.check_statuscode(unsafe { tcl_sys::Tcl_Eval(self.interp_ptr()?, c_code.as_ptr()) })?;

        self.get_result().map(|obj| obj.to_string())
    }

    pub fn call<'a, I>(&mut self, objv: I) -> PyResult<String>
    where
        I: IntoIterator<Item = &'a PyAny>,
    {
        let objv = Objv::new(self, objv)?;

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_EvalObjv(self.interp_ptr()?, objv.len(), objv.as_ptr(), 0)
        })?;

        self.get_result().map(|obj| obj.to_string())
    }

    pub fn get_result(&self) -> PyResult<TclObj> {
        NonNull::new(unsafe { tcl_sys::Tcl_GetObjResult(self.interp_ptr()?) })
            .ok_or_else(|| TclError::py_err("Tcl_GetObjResult returned NULL"))
            .map(TclObj::new)
    }

    pub fn set_result(&mut self, obj: TclObj) -> PyResult<()> {
        unsafe { tcl_sys::Tcl_SetObjResult(self.interp_ptr()?, obj.as_ptr()) }
        Ok(())
    }

    pub fn get_error(&self) -> PyResult<PyErr> {
        Ok(TclError::py_err(self.get_result()?.to_string()))
    }

    pub fn check_statuscode(&self, value: c_int) -> PyResult<()> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ => Err(self.get_error()?),
        }
    }

    pub(crate) fn make_string_obj(&self, arg: &PyAny) -> PyResult<TclObj> {
        if let Ok(s) = arg.downcast_ref::<PyString>() {
            Ok(TclObj::from(s.as_bytes()))
        } else if let Ok(t) = arg.downcast_ref::<PyTuple>() {
            let objv = Objv::new(self, t)?;

            let ptr = unsafe { tcl_sys::Tcl_NewListObj(objv.len(), objv.as_ptr()) };
            let ptr = NonNull::new(ptr)
                .ok_or_else(|| TclError::py_err("Tcl_NewListObj returned NULL"))?;

            Ok(TclObj::new(ptr))
        } else {
            Err(pyo3::exceptions::TypeError::py_err("Expected str or tuple"))
        }
    }

    pub fn splitlist(&mut self, arg: &PyString) -> PyResult<Vec<String>> {
        let obj = self.make_string_obj(arg.as_ref())?;

        let mut objc: c_int = 0;
        let mut objv: *mut *mut tcl_sys::Tcl_Obj = std::ptr::null_mut();

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_ListObjGetElements(self.interp_ptr()?, obj.as_ptr(), &mut objc, &mut objv)
        })?;

        Ok(unsafe { slice::from_raw_parts(objv, objc as usize) }
            .into_iter()
            .map(|&ptr| TclObj::new(NonNull::new(ptr).unwrap()).to_string())
            .collect::<Vec<_>>())
    }

    pub fn delete(&mut self) -> PyResult<()> {
        unsafe { tcl_sys::Tcl_DeleteInterp(self.interp_ptr()?) };
        self.0.lock().unwrap().interp = None;
        Ok(())
    }
}

// We must implement drop on `TclInterpData` and not `TclInterp` because otherwise we try to drop
// stuff at the same time in different instances and demons spawn.
impl Drop for TclInterpData {
    fn drop(&mut self) {
        if self.interp.is_some() {
            unsafe { tcl_sys::Tcl_DeleteInterp(self.interp.unwrap().as_ptr()) };
        }

        self.commands
            .values()
            .for_each(|&ptr| std::mem::drop(unsafe { Box::from_raw(ptr) }))
    }
}
