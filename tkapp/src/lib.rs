use std::sync::Once;

use pyo3::{create_exception, prelude::*, types::*, wrap_pyfunction};

use tclinterp::{TclInterp, ToTclObj};

#[pyclass]
pub struct TkApp {
    interp: TclInterp,
}

static LOGGER_INIT: Once = Once::new();

impl TkApp {
    pub fn new() -> PyResult<Self> {
        LOGGER_INIT.call_once(env_logger::init);

        let mut inst = Self {
            interp: TclInterp::new().map_err(|err| TclError::py_err(err.0))?,
        };
        inst.interp
            .init_tk()
            .map_err(|err| TclError::py_err(err.0))?;
        Ok(inst)
    }
}

#[pymethods]
impl TkApp {
    #[args(args = "*")]
    fn call(&mut self, args: &PyTuple) -> PyResult<String> {
        self.interp
            .call(args)
            .map_err(|err| TclError::py_err(err.0))
    }

    fn eval(&mut self, code: String) -> PyResult<String> {
        self.interp
            .eval(code)
            .map_err(|err| TclError::py_err(err.0))
    }

    fn splitlist(&mut self, arg: &PyString) -> PyResult<Vec<String>> {
        self.interp
            .splitlist(arg)
            .map_err(|err| TclError::py_err(err.0))
    }

    fn getboolean(&mut self, arg: &PyString) -> PyResult<bool> {
        self.interp
            .getboolean(arg.to_string()?.to_string())
            .map_err(|err| TclError::py_err(err.0))
    }

    fn delete(&mut self) -> PyResult<()> {
        self.interp.delete().map_err(|err| TclError::py_err(err.0))
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
            .map_err(|err| TclError::py_err(err.0))
    }

    fn deletecommand(&mut self, name: &str) -> PyResult<()> {
        self.interp
            .deletecommand(name)
            .map_err(|err| TclError::py_err(err.0))
    }

    fn mainloop(&mut self, _arg: &PyAny) -> PyResult<()> {
        self.interp
            .mainloop()
            .map_err(|err| TclError::py_err(err.0))
    }
}

fn errmsg(py: Python, err: &PyErr) -> String {
    use pyo3::PyErrValue;

    match &err.pvalue {
        PyErrValue::None => panic!("No error message"),

        PyErrValue::Value(obj) => obj.extract::<String>(py).unwrap(),

        PyErrValue::ToArgs(args) => args.arguments(py).extract::<String>(py).unwrap(),

        PyErrValue::ToObject(obj_candidate) => {
            obj_candidate.to_object(py).extract::<String>(py).unwrap()
        }
    }
}

create_exception!(mystcl, TclError, pyo3::exceptions::Exception);

/// Create a new `TkApp` instance.
#[pyfunction]
fn create() -> PyResult<Py<TkApp>> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let obj = Py::new(py, TkApp::new()?)?;
    Ok(obj)
}

#[pymodule]
fn mystcl(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(create))?;
    m.add("TclError", py.get_type::<TclError>())?;
    Ok(())
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

    macro_rules! pytuple {
        ($py:expr => [$($arg:expr),*]) => {
            &PyTuple::new($py, vec![$($arg),*])
        }
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
            unwrap_pyerr!(app.call(pytuple!(py => ["foo", "bar", "baz"]))),
            "('bar', 'baz')"
        );
    }
}
