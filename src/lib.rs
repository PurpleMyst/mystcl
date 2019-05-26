#![deny(unused_imports, unused_must_use)]
#![allow(clippy::identity_conversion)] // Because clippy complains about pyo3.

// FIXME: Use a custom-built type instead of `CStr` to handle strings containing NUL bytes.

use pyo3::{prelude::*, wrap_pyfunction};

mod exceptions;
mod tclinterp;
mod tclobj;
mod tclsocket;
mod tkapp;
mod wrappers;

pub use tclinterp::TclInterp;
pub use tclobj::{TclObj, ToTclObj};

use tkapp::TkApp;

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
    m.add("TclError", py.get_type::<exceptions::py::TclError>())?;
    Ok(())
}
