use std::{
    ffi::CString,
    io::{self, Read, Write},
    os::raw::*,
    ptr,
};

use crate::{
    channel::{Channel, ChannelHandlers, ChannelOption, TranslationMode},
    exceptions::TclError,
    tclinterp::TclInterp,
    tclobj::TclObj,
};

/// A wrapper around a Tcl socket that allows Read/Write trait usage.
pub struct TclSocket {
    interp: TclInterp,
    channel_id: tcl_sys::Tcl_Channel,
    handlers: ChannelHandlers,
}

impl Drop for TclSocket {
    fn drop(&mut self) {
        self.close().unwrap();
    }
}

impl TclSocket {
    /// Connect to a specified host:port.
    pub fn connect(interp: TclInterp, host: &str, port: u16) -> Result<Self, TclError> {
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

impl Read for TclSocket {
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

impl Write for TclSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = unsafe {
            tcl_sys::Tcl_WriteChars(
                self.channel_id(),
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
        let res = unsafe { tcl_sys::Tcl_Flush(self.channel_id()) };

        self.interp()
            .check_statuscode(res)
            .map(|_| ())
            .map_err(Into::into)
    }
}

impl Channel for TclSocket {
    fn interp(&mut self) -> &mut TclInterp {
        &mut self.interp
    }

    fn channel_id(&self) -> tcl_sys::Tcl_Channel {
        self.channel_id
    }

    fn handlers(&mut self) -> &mut ChannelHandlers {
        &mut self.handlers
    }
}
