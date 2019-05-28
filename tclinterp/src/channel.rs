use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::CString,
    io::{self, Read, Write},
    mem,
    os::raw::*,
    ptr,
    rc::Rc,
};

use bitflags::bitflags;

use crate::{exceptions::TclError, tclinterp::TclInterp, tclobj::TclObj};

#[derive(Clone, Copy)]
pub enum TranslationMode {
    Binary,
}

#[derive(Clone, Copy)]
pub enum ChannelOption {
    Blocking(bool),
    TranslationMode(TranslationMode),
}

bitflags! {
    pub struct ChannelHandlerMask: u32 {
        const READABLE = tcl_sys::TCL_READABLE;
        const WRITABLE = tcl_sys::TCL_WRITABLE;
        const EXCEPTION = tcl_sys::TCL_EXCEPTION;
    }
}

pub struct ChannelHandlerData {
    pub interp: TclInterp,
    pub sock: Rc<RefCell<Channel>>,
    pub handler: ChannelHandler,
}

pub type ChannelHandler = fn(&mut ChannelHandlerData) -> ();

#[derive(Default)]
pub struct ChannelHandlers(HashSet<*mut ChannelHandlerData>);

impl Drop for ChannelHandlers {
    fn drop(&mut self) {
        self.0
            .drain()
            .map(|ptr| unsafe { Box::from_raw(ptr) })
            .for_each(mem::drop)
    }
}

impl ChannelHandlers {
    fn add(&mut self, data: *mut ChannelHandlerData) {
        self.0.insert(data);
    }
}

pub struct Channel {
    interp: TclInterp,
    channel_id: tcl_sys::Tcl_Channel,
    handlers: ChannelHandlers,
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.close().unwrap()
    }
}

impl Channel {
    #[inline]
    fn close(&mut self) -> Result<(), TclError> {
        let res = unsafe {
            tcl_sys::Tcl_Close(self.interp.interp_ptr().unwrap().as_ptr(), self.channel_id)
        };
        self.interp.check_statuscode(res)
    }

    #[inline]
    fn set_option(&mut self, option: ChannelOption) -> Result<(), TclError> {
        let (option_name, option_value) = match option {
            ChannelOption::Blocking(value) => ("-blocking", value.to_string()),
            ChannelOption::TranslationMode(mode) => (
                "-translation",
                String::from(match mode {
                    TranslationMode::Binary => "binary",
                }),
            ),
        };

        let option_name = CString::new(option_name).unwrap();
        let option_value = CString::new(option_value).unwrap();

        let res = unsafe {
            tcl_sys::Tcl_SetChannelOption(
                self.interp.interp_ptr()?.as_ptr(),
                self.channel_id,
                option_name.as_ptr(),
                option_value.as_ptr(),
            )
        };
        self.interp.check_statuscode(res)
    }
}

// socket
impl Channel {
    pub fn open_tcp_client(interp: TclInterp, host: &str, port: u16) -> Result<Self, TclError> {
        let channel_id = unsafe {
            tcl_sys::Tcl_OpenTcpClient(
                interp.interp_ptr()?.as_ptr(),
                port as c_int,
                CString::new(host).unwrap().as_ptr(),
                ptr::null(), // random local address
                0,           // random local port
                0,           // not async
            )
        };
        if channel_id.is_null() {
            return Err(interp.get_error()?);
        }

        let mut inst = Self {
            interp,
            channel_id,
            handlers: Default::default(),
        };
        inst.set_option(ChannelOption::Blocking(false))?;
        inst.set_option(ChannelOption::TranslationMode(TranslationMode::Binary))?;
        Ok(inst)
    }
}

#[inline]
pub fn add_channel_handler(
    this: Rc<RefCell<Channel>>,
    mask: ChannelHandlerMask,
    proc: ChannelHandler,
) {
    extern "C" fn tcl_channel_proc(client_data: *mut c_void, _mask: c_int) {
        let client_data = unsafe { &mut *(client_data as *mut ChannelHandlerData) };

        (client_data.handler)(client_data);
    }

    let handler_data = Box::into_raw(Box::new(ChannelHandlerData {
        interp: this.borrow_mut().interp.clone(),
        handler: proc,
        sock: this.clone(),
    }));

    this.borrow_mut().handlers.add(handler_data);

    unsafe {
        tcl_sys::Tcl_CreateChannelHandler(
            this.borrow().channel_id,
            mask.bits() as c_int,
            Some(tcl_channel_proc),
            handler_data as *mut c_void,
        )
    };
}

impl Read for Channel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut output = TclObj::empty()?;

        let res = unsafe {
            tcl_sys::Tcl_ReadChars(
                self.channel_id,
                output.as_ptr(),
                buf.len() as c_int,
                0, // don't append
            )
        };

        // FIXME: Use `Tcl_GetErrno` to return a `Result::Err`.
        if res == -1 {
            panic!("Tcl_ReadChars() returned -1");
        }

        debug_assert!(res >= 0 && res as usize <= buf.len());

        let data_bytes = output.as_bytes();
        debug_assert_eq!(res as usize, data_bytes.len());

        unsafe { ptr::copy_nonoverlapping(data_bytes.as_ptr(), buf.as_mut_ptr(), data_bytes.len()) }
        Ok(data_bytes.len())
    }
}

impl Write for Channel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = unsafe {
            tcl_sys::Tcl_WriteChars(
                self.channel_id,
                buf.as_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };

        if res == -1 {
            panic!("Tcl_WriteChars() returned -1");
        }

        debug_assert!(res >= 0 && res as usize <= buf.len());
        Ok(res as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        let res = unsafe { tcl_sys::Tcl_Flush(self.channel_id) };

        self.interp
            .check_statuscode(res)
            .map(|_| ())
            .map_err(Into::into)
    }
}
