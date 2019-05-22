use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    mem,
    os::raw::*,
    ptr::{self, NonNull},
    rc::Rc,
    slice,
    sync::Mutex,
};

use log::{debug, trace};

use crate::{
    exceptions::TclError,
    tclobj::{TclObj, ToTclObj},
    wrappers::Objv,
};

/// Access a TclInterpData attribute through the Rc<Mutex<_>>.
macro_rules! attr {
    ($self:ident.$name:ident) => {
        $self.0.lock().unwrap().$name
    };
}

mod createcommand;
use createcommand::CommandData;

mod preserve;
use preserve::Preserve;

struct TclInterpData {
    interp: NonNull<tcl_sys::Tcl_Interp>,
    commands: HashMap<CString, *mut CommandData>,
    exit_var_name: String,
}

/// A wrapper type around a `*Tcl_Interp`.
///
/// This type can be cloned to get another reference to the same interpreter. It is safe to have as
/// many of them around as you want.
///
/// Any of the methods of this struct that return a `Result` have the possibility to return an
/// `Err` if the `*Tcl_Interp` is used post-deletion.
#[derive(Clone)]
pub struct TclInterp(Rc<Mutex<TclInterpData>>);

impl TclInterp {
    /// Create a new Tcl interpreter.
    ///
    /// # Errors
    /// This method fails if `Tcl_CreateInterp()` returns a null pointer (which, as far as I can
    /// tell, should be never).
    ///
    /// It also fails if `Tcl_Init()` fails.
    pub fn new() -> Result<Self, TclError> {
        unsafe {
            // XXX: Should we move this into its own function and make it "optional"?
            let exit_var_name = format!("exit_var_{}", rand::random::<u64>());
            debug!("Creating exit variable {:?}", exit_var_name);

            let interp = Rc::new(Mutex::new(TclInterpData {
                interp: NonNull::new(tcl_sys::Tcl_CreateInterp())
                    .ok_or_else(|| TclError::new("Tcl_CreateInterp() returned NULL"))?,

                commands: Default::default(),
                exit_var_name: exit_var_name.clone(),
            }));

            let mut inst = Self(interp);

            inst.eval(String::from("rename exit {}"))?;
            inst.eval(format!("set {} false", exit_var_name))?;

            inst.check_statuscode(tcl_sys::Tcl_Init(inst.interp_ptr()?.as_ptr()))?;

            Ok(inst)
        }
    }

    /// Prepare this interpreter for Tk usage.
    pub fn init_tk(&mut self) -> Result<(), TclError> {
        self.check_statuscode(unsafe { tcl_sys::Tk_Init(self.interp_ptr()?.as_ptr()) })?;

        // XXX: Can we remove this clone?
        let exit_var_name = attr!(self.exit_var_name).clone();

        self.eval(String::from("package require Tk"))?;
        self.eval(format!("bind . <Destroy> {{ set {} true }}", exit_var_name))?;

        Ok(())
    }

    fn deleted(&self) -> bool {
        let ptr = attr!(self.interp).as_ptr();
        (unsafe { tcl_sys::Tcl_InterpDeleted(ptr) }) != 0
    }

    fn interp_ptr(&self) -> Result<Preserve<tcl_sys::Tcl_Interp>, TclError> {
        if self.deleted() {
            return Err(TclError::new("Tried to use interpreter after deletion"));
        }

        Ok(Preserve::new(attr!(self.interp)))
    }

    /// Evaluate a piece of Tcl code given as a string.
    ///
    /// # Errors
    /// This function fails if `code` contains NUL bytes or if there is an error evaluating the Tcl
    /// code.
    pub fn eval(&mut self, code: String) -> Result<String, TclError> {
        trace!("Evaluating code {:?}", code);

        let c_code =
            CString::new(code).map_err(|_| TclError::new("code must not contain NUL bytes."))?;

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_Eval(self.interp_ptr()?.as_ptr(), c_code.as_ptr())
        })?;

        let result = self.get_result()?;
        Ok(result.to_string())
    }

    /// Evaluate a piece of Tcl code given as a list.
    ///
    /// # Errors
    /// This function fails if any of the given arguments are not convertable to Tcl objects or if
    /// there is an error evaluating the resulting Tcl code.
    pub fn call<I>(&mut self, it: I) -> Result<String, TclError>
    where
        I: IntoIterator,
        I::Item: ToTclObj,
    {
        let objv = Objv::new(it);
        trace!("Calling {:?}", objv);

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_EvalObjv(self.interp_ptr()?.as_ptr(), objv.len(), objv.as_ptr(), 0)
        })?;

        let result = self.get_result()?;
        Ok(result.to_string())
    }

    fn get_result(&self) -> Result<TclObj, TclError> {
        let result_ptr = unsafe { tcl_sys::Tcl_GetObjResult(self.interp_ptr()?.as_ptr()) };

        NonNull::new(result_ptr)
            .ok_or_else(|| TclError::new("Tcl_GetObjResult() returned NULL"))
            .map(TclObj::new)
    }

    fn set_result(&mut self, obj: TclObj) -> Result<(), TclError> {
        trace!("Setting result to {:?}", obj);
        unsafe { tcl_sys::Tcl_SetObjResult(self.interp_ptr()?.as_ptr(), obj.as_ptr()) };
        Ok(())
    }

    fn get_error(&self) -> Result<TclError, TclError> {
        Ok(TclError::new(self.get_result()?.to_string()))
    }

    fn check_statuscode(&self, value: c_int) -> Result<(), TclError> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ => Err(self.get_error()?),
        }
    }

    /// Split a Tcl list object into its parts.
    ///
    /// # Errors
    /// This function fails if `arg` can not be converted to a Tcl list.
    pub fn splitlist(&self, arg: impl ToTclObj) -> Result<Vec<String>, TclError> {
        let obj = arg.to_tcl_obj();

        let mut objc: c_int = 0;
        let mut objv: *mut *mut tcl_sys::Tcl_Obj = std::ptr::null_mut();

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_ListObjGetElements(
                self.interp_ptr()?.as_ptr(),
                obj.as_ptr(),
                &mut objc,
                &mut objv,
            )
        })?;

        Ok(unsafe { slice::from_raw_parts(objv, objc as usize) }
            .iter()
            .cloned()
            .map(|ptr| NonNull::new(ptr).expect("Tcl_ListObjGetElements() returned NULL"))
            .map(|ptr| TclObj::new(ptr).to_string())
            .collect::<Vec<_>>())
    }

    /// Convert a Tcl bool to a Rust bool.
    ///
    /// # Errors
    /// This function fails if `s` contains NUL bytes or if `s` is not a Tcl bool.
    pub fn getboolean(&self, s: String) -> Result<bool, TclError> {
        let s =
            CString::new(s).map_err(|_| TclError::new("Argument must not contain NUL bytes."))?;

        let mut value: c_int = Default::default();

        self.check_statuscode(unsafe {
            tcl_sys::Tcl_GetBoolean(self.interp_ptr()?.as_ptr(), s.as_ptr(), &mut value)
        })?;

        Ok(value != 0)
    }

    /// Delete the interpreter.
    ///
    /// After this function returns, most (if not all) of the methods in this struct become
    /// unaccessible.
    pub fn delete(&mut self) -> Result<(), TclError> {
        debug!("Deleting interpreter");
        unsafe { tcl_sys::Tcl_DeleteInterp(self.interp_ptr()?.as_ptr()) };
        Ok(())
    }

    fn get_var(&self, name: &CStr) -> Result<TclObj, TclError> {
        let ptr = unsafe {
            tcl_sys::Tcl_GetVar2Ex(self.interp_ptr()?.as_ptr(), name.as_ptr(), ptr::null(), 0)
        };
        NonNull::new(ptr)
            .ok_or_else(|| TclError::new(format!("Could not get variable with name {:?}", name)))
            .map(TclObj::new)
    }

    /// Run the Tcl mainloop.
    pub fn mainloop(&mut self) -> Result<(), TclError> {
        let exit_var_name = CString::new(attr!(self.exit_var_name).clone()).unwrap();

        while !self.deleted() && self.get_var(exit_var_name.as_ref())?.to_string() != "true" {
            let res = unsafe { tcl_sys::Tcl_DoOneEvent(0) };
            assert_eq!(res, 1);
        }

        Ok(())
    }
}

// We must implement drop on `TclInterpData` and not `TclInterp` because otherwise we try to drop
// stuff at the same time in different instances and demons spawn.
impl Drop for TclInterpData {
    fn drop(&mut self) {
        unsafe {
            if (tcl_sys::Tcl_InterpDeleted(self.interp.as_ptr())) != 0 {
                tcl_sys::Tcl_DeleteInterp(self.interp.as_ptr());
            }

            self.commands
                .values()
                .cloned()
                .map(|ptr| Box::from_raw(ptr))
                .for_each(mem::drop);
        }
    }
}
