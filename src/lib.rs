#![deny(unused_imports, unused_must_use)]
#![allow(clippy::identity_conversion)] // Because clippy complains about pyo3.

// FIXME: Use a custom-built type instead of `CStr` to handle strings containing NUL bytes.

use pyo3::{prelude::*, wrap_pyfunction};

mod exceptions;
mod tclinterp;
mod tclobj;
mod tkapp;
mod wrappers;

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

#[pyfunction]
fn create() -> PyResult<Py<TkApp>> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let obj = Py::new(py, TkApp::new()?)?;
    Ok(obj)
}

#[pymodule]
fn mystcl(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(create))?;
    Ok(())
}
