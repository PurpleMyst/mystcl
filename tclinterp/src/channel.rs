use std::{
    cell::RefCell,
    collections::HashSet,
    ffi::CString,
    io::{Read, Write},
    mem,
    os::raw::*,
    rc::Rc,
};

use bitflags::bitflags;

use crate::{exceptions::TclError, tclinterp::TclInterp};

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
    pub sock: Rc<RefCell<dyn Channel>>,
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

pub trait Channel: Read + Write {
    #[inline]
    fn interp(&mut self) -> &mut TclInterp;

    #[inline]
    fn channel_id(&self) -> tcl_sys::Tcl_Channel;

    #[inline]
    fn handlers(&mut self) -> &mut ChannelHandlers;

    #[inline]
    fn close(&mut self) -> Result<(), TclError> {
        let res = unsafe {
            tcl_sys::Tcl_Close(
                self.interp().interp_ptr().unwrap().as_ptr(),
                self.channel_id(),
            )
        };
        self.interp().check_statuscode(res)
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

        let channel_id = self.channel_id();
        let interp = self.interp();
        let res = unsafe {
            tcl_sys::Tcl_SetChannelOption(
                interp.interp_ptr()?.as_ptr(),
                channel_id,
                option_name.as_ptr(),
                option_value.as_ptr(),
            )
        };
        interp.check_statuscode(res)
    }
}

#[inline]
pub fn add_channel_handler(
    this: Rc<RefCell<dyn Channel>>,
    mask: ChannelHandlerMask,
    proc: ChannelHandler,
) {
    extern "C" fn tcl_channel_proc(client_data: *mut c_void, _mask: c_int) {
        let client_data = unsafe { &mut *(client_data as *mut ChannelHandlerData) };

        (client_data.handler)(client_data);
    }

    let handler_data = Box::into_raw(Box::new(ChannelHandlerData {
        interp: this.borrow_mut().interp().clone(),
        handler: proc,
        sock: this.clone(),
    }));

    this.borrow_mut().handlers().add(handler_data);

    unsafe {
        tcl_sys::Tcl_CreateChannelHandler(
            this.borrow().channel_id(),
            mask.bits() as c_int,
            Some(tcl_channel_proc),
            handler_data as *mut c_void,
        )
    };
}

// XXX: We could implement Read + Write generically for all channels, but rustc prevents that.
