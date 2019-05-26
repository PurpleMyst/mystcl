use std::{any::Any, os::raw::*};

use super::*;

pub type Command = fn(&CommandData, &[&CStr]) -> Result<TclObj, TclObj>;

pub struct CommandData {
    pub interp: TclInterp,
    pub name: CString,
    pub cmd: Command,
    pub data: Box<Any>,
}

extern "C" fn cmd_callback(
    client_data: *mut c_void,
    _interp: *mut tcl_sys::Tcl_Interp,
    argc: c_int,
    argv: *mut *const c_char,
) -> c_int {
    // XXX: We might need to forget this!!!
    let client_data = unsafe { &mut *(client_data as *mut CommandData) };
    trace!("Calling command {:?}", client_data.name);

    let args = unsafe {
        slice::from_raw_parts(argv, argc as usize)
            .iter()
            .skip(1)
            .map(|&s| CStr::from_ptr(s))
            .collect::<Vec<_>>()
    };

    let res = (client_data.cmd)(&client_data, &args);

    match res {
        Ok(value) => {
            if !client_data.interp.deleted() {
                client_data
                    .interp
                    .set_result(value)
                    .expect("Could not set successful result from command");
            }

            tcl_sys::TCL_OK as c_int
        }

        Err(value) => {
            if !client_data.interp.deleted() {
                client_data
                    .interp
                    .set_result(value)
                    .expect("Could not set failing result from command")
            }
            tcl_sys::TCL_ERROR as c_int
        }
    }
}

extern "C" fn cmd_deleter(client_data: *mut c_void) {
    let client_data = unsafe { &mut *(client_data as *mut CommandData) };
    debug!("Deleting command {:?}", client_data.name);

    let cmd_name = &client_data.name;
    let interp = &mut client_data.interp;

    let cmd = attr!(interp.commands).remove(cmd_name);
    assert!(cmd.is_some());
}

impl TclInterp {
    pub fn createcommand(
        &mut self,
        name: &str,
        data: Box<Any>,
        cmd: Command,
    ) -> Result<(), TclError> {
        let name =
            CString::new(name).map_err(|_| TclError::new("name must not contain NUL bytes."))?;

        debug!("Creating command {:?}", name);
        debug!("Commands: {:?}", attr!(self.commands));

        if attr!(self.commands).contains_key(&name) {
            return Err(TclError::new(format!(
                "Command with name {:?} already exists.",
                name
            )));
        }

        let command_data = CommandData {
            interp: self.clone(),
            name: name.clone(),
            cmd,
            data,
        };
        let command_data = Box::into_raw(Box::new(command_data)) as *mut c_void;

        let res = unsafe {
            tcl_sys::Tcl_CreateCommand(
                self.interp_ptr()?.as_ptr(),
                name.as_ptr(),
                Some(cmd_callback),
                command_data,
                Some(cmd_deleter),
            )
        };

        if res.is_null() {
            return Err(TclError::new("Tcl_CreateCommand returned NULL"));
        }

        let old_cmd = attr!(self.commands).insert(name, command_data as *mut CommandData);
        assert!(old_cmd.is_none());

        Ok(())
    }

    pub fn deletecommand(&mut self, name: &str) -> Result<(), TclError> {
        let name =
            CString::new(name).map_err(|_| TclError::new("name must not contain NUL bytes."))?;

        let res = unsafe { tcl_sys::Tcl_DeleteCommand(self.interp_ptr()?.as_ptr(), name.as_ptr()) };

        match res {
            0 => Ok(()),
            -1 => Err(TclError::new(format!(
                "Command with name {:?} does not exist.",
                name
            ))),

            _ => unreachable!(),
        }
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
        assert_eq!(interp.eval("foo".to_string()).unwrap().to_string(), "bar");
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
                .unwrap()
                .to_string(),
            "spam ham spam spam ham ham spam"
        );
    }

    #[test]
    fn test_deletecommand() {
        let mut interp = TclInterp::new().unwrap();
        interp
            .createcommand("foo", Box::new("bar"), |_, _| Ok("hi".to_tcl_obj()))
            .unwrap();

        assert!(attr!(interp.commands).contains_key(&CString::new("foo").unwrap()));
        assert!(interp.eval("foo".to_owned()).is_ok());
        interp.deletecommand("foo").unwrap();
        assert!(!attr!(interp.commands).contains_key(&CString::new("foo").unwrap()));
        assert!(interp.eval("foo".to_owned()).is_err());
    }
}
