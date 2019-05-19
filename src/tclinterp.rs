use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::*,
    ptr::NonNull,
    rc::Rc,
    slice,
    sync::Mutex,
};

use crate::{
    exceptions::TclError,
    tclobj::{TclObj, ToTclObj},
    wrappers::Objv,
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
    pub fn new() -> Result<Self, TclError> {
        unsafe {
            let interp = Rc::new(Mutex::new(TclInterpData {
                interp: Some(
                    NonNull::new(tcl_sys::Tcl_CreateInterp())
                        .ok_or_else(|| TclError::new("Tcl_CreateInterp returned NULL"))?,
                ),
                commands: Default::default(),
            }));

            let inst = Self(interp);

            inst.check_statuscode(tcl_sys::Tcl_Init(inst.interp_ptr()?))?;

            Ok(inst)
        }
    }

    pub fn init_tk(&mut self) -> Result<(), TclError> {
        self.check_statuscode(unsafe { tcl_sys::Tk_Init(self.interp_ptr()?) })?;

        let id = &self as *const _ as usize;
        let exit_var_name = format!("exit_var_{}", id);
        self.eval(String::from("package require Tk"))?;
        self.eval(String::from("rename exit {}"))?;
        self.eval(format!("set {} false", exit_var_name))?;
        self.eval(format!("bind . <Destroy> {{ set {} true }}", exit_var_name))?;

        Ok(())
    }

    pub(crate) fn interp_ptr(&self) -> Result<*mut tcl_sys::Tcl_Interp, TclError> {
        self.0
            .lock()
            .unwrap()
            .interp
            .ok_or_else(|| TclError::new("Tried to use interpreter after deletion"))
            .map(|ptr| ptr.as_ptr())
    }

    pub fn eval(&mut self, code: String) -> Result<String, TclError> {
        let c_code =
            CString::new(code).map_err(|_| TclError::new("code must not contain NUL bytes."))?;

        self.check_statuscode(unsafe { tcl_sys::Tcl_Eval(self.interp_ptr()?, c_code.as_ptr()) })?;

        self.get_result().map(|obj| obj.to_string())
    }

    pub fn call<'a, I>(&mut self, it: I) -> Result<String, TclError>
    where
        I: IntoIterator,
        I::Item: ToTclObj,
    {
        let objv = Objv::new(it);

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_EvalObjv(self.interp_ptr()?, objv.len(), objv.as_ptr(), 0)
        })?;

        self.get_result().map(|obj| obj.to_string())
    }

    pub fn get_result(&self) -> Result<TclObj, TclError> {
        NonNull::new(unsafe { tcl_sys::Tcl_GetObjResult(self.interp_ptr()?) })
            .ok_or_else(|| TclError::new("Tcl_GetObjResult returned NULL"))
            .map(TclObj::new)
    }

    pub fn set_result(&mut self, obj: TclObj) -> Result<(), TclError> {
        unsafe { tcl_sys::Tcl_SetObjResult(self.interp_ptr()?, obj.as_ptr()) }
        Ok(())
    }

    pub fn get_error(&self) -> Result<TclError, TclError> {
        Ok(TclError::new(self.get_result()?.to_string()))
    }

    pub fn check_statuscode(&self, value: c_int) -> Result<(), TclError> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ => Err(self.get_error()?),
        }
    }

    pub fn splitlist(&mut self, arg: impl ToTclObj) -> Result<Vec<String>, TclError> {
        let obj = arg.to_tcl_obj();

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

    pub fn delete(&mut self) -> Result<(), TclError> {
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
