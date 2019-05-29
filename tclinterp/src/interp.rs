use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString},
    io::{self, Write},
    mem,
    net::TcpStream,
    os::raw::*,
    ptr::{self, NonNull},
    rc::Rc,
    slice,
    sync::Arc,
    sync::Mutex,
    thread,
};

use log::{debug, trace};

use crate::{
    channel::{add_channel_handler, ChannelHandlerMask},
    error::{Result, TclError},
    obj::{TclObj, ToTclObj},
    postoffice::{TclRequest, TclResponse},
    utils::{preserve::Preserve, socketpair::create_socketpair},
    wrappers::Objv,
};

/// Access a TclInterpData attribute through the Arc<Mutex<_>>.
macro_rules! attr {
    ($self:ident.$name:ident) => {
        $self.0.lock().unwrap().$name
    };
}

macro_rules! communicate {
    ($self:ident: $request:expr => $response:pat => $result:expr) => {{
        let safety_sock_attr = { attr!($self.safety_sock).clone() };

        let mut safety_sock_mutex = safety_sock_attr.lock().unwrap();
        let mut safety_sock = safety_sock_mutex
            .as_mut()
            .expect("Can not call methods from other threads before init_threads()");

        bincode::serialize_into(&mut safety_sock, &$request).unwrap();
        safety_sock.flush().unwrap();

        if let $response = bincode::deserialize_from(&mut safety_sock).unwrap() {
            $result
        } else {
            unreachable!()
        }
    }};
}

mod createcommand;
use createcommand::CommandData;

struct TclInterpData {
    interp: NonNull<tcl_sys::Tcl_Interp>,
    commands: HashMap<CString, *mut CommandData>,
    exit_var_name: String,

    owner: thread::ThreadId,

    // we use another Arc<Mutex<_>> here so that the whole interpreter can be used even while this
    // sock is held.
    safety_sock: Arc<Mutex<Option<TcpStream>>>,
}

unsafe impl Send for TclInterpData {}
unsafe impl Sync for TclInterpData {}

/// A wrapper type around a `*Tcl_Interp`.
///
/// This type can be cloned to get another reference to the same interpreter. It is safe to have as
/// many of them around as you want.
///
/// Any of the methods of this struct that return a `Result` have the possibility to return an
/// `Err` if the `*Tcl_Interp` is used post-deletion.
#[derive(Clone)]
pub struct TclInterp(Arc<Mutex<TclInterpData>>);

impl TclInterp {
    /// Create a new Tcl interpreter.
    ///
    /// # Errors
    /// This method fails if `Tcl_CreateInterp()` returns a null pointer (which, as far as I can
    /// tell, should be never).
    ///
    /// It also fails if `Tcl_Init()` fails.
    pub fn new() -> Result<Self> {
        unsafe {
            // XXX: Should we move this into its own function and make it "optional"?
            let exit_var_name = format!("exit_var_{}", rand::random::<u64>());
            debug!("Creating exit variable {:?}", exit_var_name);

            let interp = Arc::new(Mutex::new(TclInterpData {
                interp: NonNull::new(tcl_sys::Tcl_CreateInterp())
                    .ok_or_else(|| TclError::new("Tcl_CreateInterp() returned NULL"))?,

                commands: Default::default(),
                exit_var_name: exit_var_name.clone(),

                owner: thread::current().id(),
                safety_sock: Default::default(),
            }));

            let mut inst = Self(interp);

            inst.eval(String::from("rename exit {}"))?;
            inst.eval(format!("set {} false", exit_var_name))?;

            inst.check_statuscode(tcl_sys::Tcl_Init(inst.interp_ptr()?.as_ptr()))?;

            Ok(inst)
        }
    }

    /// Prepare this interpreter for Tk usage.
    pub fn init_tk(&mut self) -> Result<()> {
        self.check_statuscode(unsafe { tcl_sys::Tk_Init(self.interp_ptr()?.as_ptr()) })?;

        // XXX: Can we remove this clone?
        let exit_var_name = attr!(self.exit_var_name).clone();

        self.eval(String::from("package require Tk"))?;
        self.eval(format!("bind . <Destroy> {{ set {} true }}", exit_var_name))?;

        Ok(())
    }

    pub fn init_threads(&mut self) -> Result<()> {
        let (rsock, tclsock) = create_socketpair(self.clone())?;

        add_channel_handler(
            Rc::new(RefCell::new(tclsock)),
            ChannelHandlerMask::READABLE,
            |handler_data| {
                let mut sock = handler_data.sock.borrow_mut();

                let request = match bincode::deserialize_from(&mut *sock) {
                    Ok(msg) => msg,
                    Err(err) => match *err {
                        bincode::ErrorKind::Io(ref err)
                            if err.kind() == io::ErrorKind::UnexpectedEof =>
                        {
                            return;
                        }
                        _ => panic!("{:?}", err),
                    },
                };

                match request {
                    TclRequest::Eval(code) => {
                        let result = handler_data
                            .interp
                            .clone() // TODO: remove this clone later
                            .eval(code)
                            .map(|obj| obj.to_string())
                            .map_err(|err| err.to_string());

                        bincode::serialize_into(&mut *sock, &TclResponse::Eval(result))
                            .map_err(|err| err.to_string().to_tcl_obj())
                            .unwrap();

                        sock.flush()
                            .map_err(|err| err.to_string().to_tcl_obj())
                            .unwrap();
                    }
                }
            },
        );

        attr!(self.safety_sock) = Arc::new(Mutex::new(Some(rsock)));

        Ok(())
    }

    pub fn deleted(&self) -> bool {
        let ptr = attr!(self.interp).as_ptr();
        (unsafe { tcl_sys::Tcl_InterpDeleted(ptr) }) != 0
    }

    pub(crate) fn interp_ptr(&self) -> Result<Preserve<tcl_sys::Tcl_Interp>> {
        debug_assert_eq!(thread::current().id(), attr!(self.owner));

        if self.deleted() {
            return Err(TclError::new("Tried to use interpreter after deletion"));
        }

        Ok(Preserve::new(attr!(self.interp)))
    }

    #[inline]
    fn is_main_thread(&self) -> bool {
        thread::current().id() == attr!(self.owner)
    }

    /// Evaluate a piece of Tcl code given as a string.
    ///
    /// # Errors
    /// This function fails if `code` contains NUL bytes or if there is an error evaluating the Tcl
    /// code.
    // FIXME: make TclInterp::Eval take a ToTclObj
    pub fn eval(&mut self, code: String) -> Result<TclObj> {
        trace!("Evaluating code {:?}", code);

        if self.is_main_thread() {
            let c_code = CString::new(code)
                .map_err(|_| TclError::new("code must not contain NUL bytes."))?;

            self.check_statuscode(unsafe {
                tcl_sys::Tcl_Eval(self.interp_ptr()?.as_ptr(), c_code.as_ptr())
            })?;

            self.get_result()
        } else {
            communicate!(self: TclRequest::Eval(code) => TclResponse::Eval(result) => {
                result.map(|ref ok| ok.to_tcl_obj()).map_err(TclError::new)
            })
        }
    }

    /// Evaluate a piece of Tcl code given as a list.
    ///
    /// # Errors
    /// This function fails if any of the given arguments are not convertable to Tcl objects or if
    /// there is an error evaluating the resulting Tcl code.
    pub fn call<I>(&mut self, it: I) -> Result<TclObj>
    where
        I: IntoIterator,
        I::Item: ToTclObj,
    {
        if self.is_main_thread() {
            let objv = Objv::new(it);
            trace!("Calling {:?}", objv);

            self.check_statuscode(unsafe {
                tcl_sys::Tcl_EvalObjv(self.interp_ptr()?.as_ptr(), objv.len(), objv.as_ptr(), 0)
            })?;

            self.get_result()
        } else {
            unimplemented!();
        }
    }

    pub(crate) fn get_result(&self) -> Result<TclObj> {
        let result_ptr = unsafe { tcl_sys::Tcl_GetObjResult(self.interp_ptr()?.as_ptr()) };

        NonNull::new(result_ptr)
            .ok_or_else(|| TclError::new("Tcl_GetObjResult() returned NULL"))
            .map(TclObj::new)
    }

    pub(crate) fn set_result(&mut self, obj: TclObj) -> Result<()> {
        trace!("Setting result to {:?}", obj);
        unsafe { tcl_sys::Tcl_SetObjResult(self.interp_ptr()?.as_ptr(), obj.as_ptr()) };
        Ok(())
    }

    pub(crate) fn get_error(&self) -> Result<TclError> {
        Ok(TclError::new(self.get_result()?.to_string()))
    }

    pub(crate) fn check_statuscode(&self, value: c_int) -> Result<()> {
        match value as c_uint {
            tcl_sys::TCL_OK => Ok(()),
            _ => Err(self.get_error()?),
        }
    }

    /// Split a Tcl list object into its parts.
    ///
    /// # Errors
    /// This function fails if `arg` can not be converted to a Tcl list.
    pub fn splitlist(&self, arg: impl ToTclObj) -> Result<Vec<String>> {
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
    pub fn getboolean(&self, s: String) -> Result<bool> {
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
    pub fn delete(&mut self) -> Result<()> {
        debug!("Deleting interpreter");
        unsafe { tcl_sys::Tcl_DeleteInterp(self.interp_ptr()?.as_ptr()) };
        Ok(())
    }

    fn get_var(&self, name: &CStr) -> Result<TclObj> {
        let ptr = unsafe {
            tcl_sys::Tcl_GetVar2Ex(self.interp_ptr()?.as_ptr(), name.as_ptr(), ptr::null(), 0)
        };
        NonNull::new(ptr)
            .ok_or_else(|| TclError::new(format!("Could not get variable with name {:?}", name)))
            .map(TclObj::new)
    }

    /// Run the Tcl mainloop.
    pub fn mainloop(&mut self) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // FIXME. this thread illustrates some lock contetion issues. somebody who is waiting on a
    // response should not lock the thread.
    #[test]
    fn test_init_threads_once() {
        let mut interp = TclInterp::new().unwrap();
        interp.init_threads().unwrap();

        let barrier = Arc::new(std::sync::Barrier::new(2));

        let child1 = {
            let mut interp = interp.clone();
            let barrier = barrier.clone();

            std::thread::spawn(move || {
                barrier.wait();

                let a = interp
                    .eval("format %s 4".to_owned())
                    .map(|obj| obj.to_string())
                    .unwrap();

                let b = interp
                    .eval("format %s 2".to_owned())
                    .map(|obj| obj.to_string())
                    .unwrap();

                a + &b
            })
        };

        let fuckyou = attr!(interp.exit_var_name).clone();
        barrier.wait();
        interp
            .call(&["after", "100", "set", &fuckyou, "true"])
            .unwrap();
        interp.mainloop().unwrap();

        let res = child1.join().unwrap();
        assert_eq!(res.to_string(), "42");
    }

    #[test]
    fn test_init_threads_twice() {
        let mut interp = TclInterp::new().unwrap();
        interp.init_threads().unwrap();

        let barrier = Arc::new(std::sync::Barrier::new(3));

        let child1 = {
            let mut interp = interp.clone();
            let barrier = barrier.clone();

            std::thread::spawn(move || {
                barrier.wait();
                interp
                    .eval("format %s 42".to_owned())
                    .map(|obj| obj.to_string())
            })
        };

        let child2 = {
            let mut interp = interp.clone();
            let barrier = barrier.clone();

            std::thread::spawn(move || {
                barrier.wait();
                interp
                    .eval("format %s 69".to_owned())
                    .map(|obj| obj.to_string())
            })
        };

        let fuckyou = attr!(interp.exit_var_name).clone();
        barrier.wait();
        interp
            .call(&["after", "100", "set", &fuckyou, "true"])
            .unwrap();
        interp.mainloop().unwrap();

        let res = child1.join().unwrap().unwrap();
        assert_eq!(res.to_string(), "42");

        let res = child2.join().unwrap().unwrap();
        assert_eq!(res.to_string(), "69");
    }
}
