use std::os::raw::*;

use pyo3::prelude::*;

use crate::wrappers::TclObjWrapper;

use super::*;

pub type Command = fn(&mut TclInterp, &[&CStr]) -> TclObjWrapper;

// XXX: This leaks memory
pub(super) struct CommandData(TclInterp, Command);

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
            .map(|&s| CStr::from_ptr(s))
            .collect::<Vec<_>>()
    };

    let obj = client_data.1(&mut client_data.0, args.as_slice());
    client_data.0.set_result(obj).unwrap();

    return tcl_sys::TCL_OK as c_int;
}

impl TclInterp {
    pub fn createcommand(&mut self, name: &str, cmd: Command) -> PyResult<()> {
        let name = CString::new(name)?;

        if self.0.lock().unwrap().commands.contains_key(&name) {
            return Err(TclError::py_err(format!(
                "Command with name {:?} already exists.",
                name
            )));
        }

        let command_data = CommandData(self.clone(), cmd);
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
            panic!("FUCK ME");
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

    #[test]
    fn test_createcommand() {
        let mut interp = TclInterp::new().unwrap();
        interp
            .createcommand("foo", |_interp, _args| {
                TclObjWrapper::try_from_string("bar".to_string()).unwrap()
            })
            .unwrap();
        assert_eq!(interp.eval("foo".to_string()).unwrap(), "bar");
    }
}
