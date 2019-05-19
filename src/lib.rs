#![deny(unused_must_use)]

use pyo3::{prelude::*, wrap_pyfunction};

mod exceptions;
mod tclinterp;
mod tkapp;
mod wrappers;

use tkapp::TkApp;

#[cfg(test)]
fn errmsg(py: Python, err: &PyErr) -> String {
    match &err.pvalue {
        pyo3::PyErrValue::ToObject(obj_candidate) => {
            obj_candidate.to_object(py).extract::<String>(py).unwrap()
        }
        _ => unimplemented!(),
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
