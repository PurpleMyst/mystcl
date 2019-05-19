#[derive(Debug, Clone)]
pub struct TclError(pub String);

mod py {
    use pyo3::create_exception;

    create_exception!(mystcl, TclError, pyo3::exceptions::Exception);
}

impl From<TclError> for pyo3::PyErr {
    fn from(err: TclError) -> Self {
        py::TclError::py_err(err.0)
    }
}
