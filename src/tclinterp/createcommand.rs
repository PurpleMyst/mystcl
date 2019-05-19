// TODO: Do something with deleters.
use std::{any::Any, os::raw::*};

use pyo3::prelude::*;

use super::*;

pub type Command = fn(&CommandData, &[&CStr]) -> Result<TclObj, TclObj>;

pub struct CommandData {
    pub interp: TclInterp,
    pub cmd: Command,
    pub data: Box<Any>,
}

#[allow(dead_code)]
extern "C" fn cmd_callback(
    client_data: *mut c_void,
    _interp: *mut tcl_sys::Tcl_Interp,
    argc: c_int,
    argv: *mut *const c_char,
) -> c_int {
    let client_data = unsafe { &mut *(client_data as *mut CommandData) };

    let args = unsafe {
        slice::from_raw_parts(argv, argc as usize)
            .into_iter()
            .skip(1)
            .map(|&s| CStr::from_ptr(s))
            .collect::<Vec<_>>()
    };

    let res = (client_data.cmd)(&client_data, &args);

    match res {
        Ok(value) => {
            client_data.interp.set_result(value).unwrap();
            tcl_sys::TCL_OK as c_int
        }

        Err(value) => {
            client_data.interp.set_result(value).unwrap();
            tcl_sys::TCL_ERROR as c_int
        }
    }
}

impl TclInterp {
    pub fn createcommand(&mut self, name: &str, data: Box<Any>, cmd: Command) -> PyResult<()> {
        let name = CString::new(name)?;

        if self.0.lock().unwrap().commands.contains_key(&name) {
            return Err(TclError::py_err(format!(
                "Command with name {:?} already exists.",
                name
            )));
        }

        let command_data = CommandData {
            interp: self.clone(),
            cmd,
            data,
        };
        let command_data = Box::into_raw(Box::new(command_data)) as *mut c_void;

        let res = unsafe {
            tcl_sys::Tcl_CreateCommand(
                self.interp_ptr()?,
                name.as_ptr(),
                Some(cmd_callback),
                command_data,
                None,
            )
        };

        if res.is_null() {
            return Err(TclError::py_err("Tcl_CreateCommand returned NULL"));
        }

        let old = self
            .0
            .lock()
            .unwrap()
            .commands
            .insert(name, command_data as *mut CommandData);
        debug_assert!(old.is_none());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add tests for a few more things.

    #[test]
    fn test_createcommand_data() {
        let mut interp = TclInterp::new().unwrap();
        interp
            .createcommand("foo", Box::new("bar".to_string()), |data, _args| {
                data.data
                    .downcast_ref::<String>()
                    .map(|s| s.to_tcl_obj())
                    .ok_or_else(|| unreachable!())
            })
            .unwrap();
        assert_eq!(interp.eval("foo".to_string()).unwrap(), "bar");
    }

    #[test]
    fn test_createcommand_args() {
        let mut interp = TclInterp::new().unwrap();
        interp
            .createcommand("ham", Box::new("unused"), |data, args| {
                assert_eq!(data.data.downcast_ref::<&str>(), Some(&"unused"));

                args.into_iter()
                    .map(|s| s.to_str().to_owned())
                    .collect::<Result<Vec<_>, _>>()
                    .or_else(|_| unreachable!())
                    .map(|v| v.join(" ").to_tcl_obj())
            })
            .unwrap();
        assert_eq!(
            interp
                .eval("ham spam ham spam spam ham ham spam".to_string())
                .map_err(|err| {
                    let gil = Python::acquire_gil();
                    crate::errmsg(gil.python(), &err)
                })
                .unwrap(),
            "spam ham spam spam ham ham spam"
        );
    }
}
