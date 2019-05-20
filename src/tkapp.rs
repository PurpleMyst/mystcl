use pyo3::{
    prelude::*,
    types::{PyAny, PyString, PyTuple},
};

use crate::{tclinterp::TclInterp, tclobj::ToTclObj};

#[pyclass]
pub struct TkApp {
    interp: TclInterp,
}

impl TkApp {
    pub fn new() -> PyResult<Self> {
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

    #[test]
    fn test_new() {
        assert!(TkApp::new().is_ok());
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

        let mut app = TkApp::new().expect("Could not create TkApp");

        assert_eq!(
            app.call(&pytuple!(py, ["return", "hello, world"])).unwrap(),
            "hello, world"
        );
    }

    #[test]
    fn test_delete() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = TkApp::new().expect("Could not create TkApp");
        app.delete().expect("Could not delete interpeter");

        if let Err(err) = app.call(&pytuple!(py, ["return", "test123"])) {
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
            TkApp::new().unwrap().eval("return 42".to_owned()).unwrap(),
            "42"
        );
    }

    #[test]
    fn test_createcommand() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let func = py.eval("lambda *args: str(args)", None, None).unwrap();
        let func = func.extract::<Py<PyAny>>().unwrap();

        let mut app = TkApp::new().unwrap();

        app.createcommand("foo", func).unwrap();

        assert_eq!(
            app.call(&pytuple!(py, ["foo", "bar", "baz"]))
                .map_err(|e| crate::errmsg(py, &e))
                .unwrap(),
            "('bar', 'baz')"
        );
    }

    #[test]
    fn test_splitlist() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = TkApp::new().unwrap();

        let l1 = app
            .call(pytuple!(py, ["list", "a", "b", "c and d"]))
            .unwrap();

        let l1_tuple_py = PyString::new(py, &l1);
        let l1_tuple = l1_tuple_py.as_ref(py);

        let mut l1_parts = app.splitlist(&l1_tuple).unwrap();
        l1_parts.insert(0, "list".to_owned());

        let l2 = app.call(&PyTuple::new(py, l1_parts).as_ref(py)).unwrap();

        assert_eq!(l1, l2);
    }
}
