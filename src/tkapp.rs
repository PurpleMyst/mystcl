use pyo3::{prelude::*, types::PyTuple};

use crate::tclinterp::TclInterp;

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
        self.interp.call(args)
    }

    fn delete(&mut self) -> PyResult<()> {
        self.interp.delete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pyo3::PyErrValue;

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

    fn errmsg(py: Python, err: &PyErr) -> String {
        match &err.pvalue {
            PyErrValue::ToObject(obj_candidate) => {
                obj_candidate.to_object(py).extract::<String>(py).unwrap()
            }
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_delete() {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut app = TkApp::new().expect("Could not create TkApp");
        app.delete().expect("Could not delete interpeter");

        if let Err(err) = app.call(&pytuple!(py, ["return", "test123"])) {
            assert_eq!(errmsg(py, &err), "Tried to use interpreter after deletion");
        } else {
            panic!("TkApp::call did not return Err(_) after TkApp::delete");
        }
    }
}
