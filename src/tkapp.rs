use std::sync::Once;

use pyo3::{prelude::*, types::*};

use crate::{tclinterp::TclInterp, tclobj::ToTclObj};

#[pyclass]
pub struct TkApp {
    interp: TclInterp,
}

static LOGGER_INIT: Once = Once::new();

impl TkApp {
    pub fn new() -> PyResult<Self> {
        LOGGER_INIT.call_once(|| env_logger::init());

        let mut inst = Self {
            interp: TclInterp::new()?,
        };
        inst.interp.init_tk()?;
        Ok(inst)
    }
}

#[pymethods]
impl TkApp {
    #[args(args = "*")]
    fn call(&mut self, args: &PyTuple) -> PyResult<String> {
        self.interp.call(args).map_err(Into::into)
    }

    fn eval(&mut self, code: String) -> PyResult<String> {
        self.interp.eval(code).map_err(Into::into)
    }

    fn splitlist(&mut self, arg: &PyString) -> PyResult<Vec<String>> {
        self.interp.splitlist(arg).map_err(Into::into)
    }

    fn getboolean(&mut self, arg: &PyString) -> PyResult<bool> {
        self.interp
            .getboolean(arg.to_string()?.to_string())
            .map_err(Into::into)
    }

    fn delete(&mut self) -> PyResult<()> {
        self.interp.delete().map_err(Into::into)
    }

    fn createcommand(&mut self, name: &str, func: Py<PyAny>) -> PyResult<()> {
        // TODO: Better errors here.
        self.interp
            .createcommand(name, Box::new(func), |cmd_data, args| {
                let gil = Python::acquire_gil();
                let py = gil.python();

                let func = cmd_data
                    .data
                    .downcast_ref::<Py<PyAny>>()
                    .unwrap()
                    .to_owned();

                let args = args
                    .iter()
                    .map(|s| s.to_str())
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();

                func.to_object(py)
                    .call(py, PyTuple::new(py, args), None)
                    .map(|v| v.as_ref(py).to_tcl_obj())
                    .map_err(|e| crate::errmsg(py, &e).to_tcl_obj())
            })
            .map_err(Into::into)
    }

    fn deletecommand(&mut self, name: &str) -> PyResult<()> {
        self.interp.deletecommand(name).map_err(Into::into)
    }

    fn mainloop(&mut self, _arg: &PyAny) -> PyResult<()> {
        self.interp.mainloop().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errmsg;

    macro_rules! unwrap_pyerr {
        ($x:expr) => {
            $x.map_err(|e| errmsg(Python::acquire_gil().python(), &e))
                .unwrap()
        };
    }

    #[test]
    fn test_new() {
        unwrap_pyerr!(TkApp::new());
    }

    macro_rules! pytuple {
        ($py:expr, [$($arg:expr),*]) => {
            &PyTuple::new($py, vec![$($arg),*]).as_ref($py)
        }
    }

    #[test]
    fn test_call() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = unwrap_pyerr!(TkApp::new());

        assert_eq!(
            unwrap_pyerr!(app.call(&pytuple!(py, ["format", "%s", "hello, world"]))),
            "hello, world"
        );
    }

    #[test]
    fn test_delete() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = unwrap_pyerr!(TkApp::new());
        unwrap_pyerr!(app.delete());

        if let Err(err) = app.call(&pytuple!(py, ["format", "%s", "test123"])) {
            assert_eq!(
                crate::errmsg(py, &err),
                "Tried to use interpreter after deletion"
            );
        } else {
            panic!("TkApp::call did not return Err(_) after TkApp::delete");
        }
    }

    #[test]
    fn test_eval() {
        assert_eq!(
            unwrap_pyerr!(unwrap_pyerr!(TkApp::new()).eval("format %s {42}".to_owned())),
            "42"
        );
    }

    #[test]
    fn test_createcommand() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let func = unwrap_pyerr!(py.eval("lambda *args: str(args)", None, None));
        let func = unwrap_pyerr!(func.extract::<Py<PyAny>>());

        let mut app = unwrap_pyerr!(TkApp::new());

        unwrap_pyerr!(app.createcommand("foo", func));

        assert_eq!(
            unwrap_pyerr!(app.call(&pytuple!(py, ["foo", "bar", "baz"]))),
            "('bar', 'baz')"
        );
    }

    #[test]
    fn test_splitlist() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = unwrap_pyerr!(TkApp::new());

        let l1 = unwrap_pyerr!(app.call(pytuple!(py, ["list", "a", "b", "c and d"])));

        let l1_tuple_py = PyString::new(py, &l1);
        let l1_tuple = l1_tuple_py.as_ref(py);

        let mut l1_parts = unwrap_pyerr!(app.splitlist(&l1_tuple));
        l1_parts.insert(0, "list".to_owned());

        let l2 = unwrap_pyerr!(app.call(&PyTuple::new(py, l1_parts).as_ref(py)));

        assert_eq!(l1, l2);
    }
}
