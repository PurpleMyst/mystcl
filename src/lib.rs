use pyo3::{prelude::*, wrap_pyfunction};

#[pyclass]
struct TkApp {

}

impl TkApp {
    fn new() -> Self {
        TkApp {}
    }
}

#[pyfunction]
fn create() -> PyResult<Py<TkApp>> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let obj = Py::new(py, TkApp::new())?;
    Ok(obj)
}

#[pymodule]
fn mystcl(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(create))?;
    Ok(())
}
