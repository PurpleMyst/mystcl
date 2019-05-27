use std::{
    io::{self, Read, Write},
    ptr,
};

use crate::{exceptions::TclError, tclinterp::TclInterp};

/// A wrapper around a Tcl socket that allows Read/Write trait usage.
pub struct TclSocket {
    interp: TclInterp,
    id: String,
}

impl Drop for TclSocket {
    fn drop(&mut self) {
        self.interp.call(&["close", &self.id]).unwrap();
    }
}

impl TclSocket {
    /// Connect to a specified host:port.
    pub fn connect(mut interp: TclInterp, host: &str, port: &str) -> Result<Self, TclError> {
        let id = interp
            .call(&["socket", host, &port.to_string()])?
            .to_string();
        let mut inst = Self { interp, id };
        inst.fconfigure("blocking", "false")?;
        inst.fconfigure("translation", "binary")?;
        Ok(inst)
    }

    fn fconfigure(&mut self, key: &str, value: &str) -> Result<(), TclError> {
        self.interp
            .call(&["fconfigure", &self.id, &format!("-{}", key), value])
            .map(|_| ())
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }
}

impl Read for TclSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let data = self
            .interp
            .call(&["read", &self.id, &buf.len().to_string()])?;

        let data_bytes = data.as_bytes();

        unsafe { ptr::copy_nonoverlapping(data_bytes.as_ptr(), buf.as_mut_ptr(), data_bytes.len()) }

        Ok(data_bytes.len())
    }
}

impl Write for TclSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.interp
            .call(objv!["puts", "-nonewline", &self.id, buf.to_tcl_obj()])
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.interp.call(&["flush", &self.id])?;

        Ok(())
    }
}
