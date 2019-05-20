use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct TclError(pub Cow<'static, str>);

impl TclError {
    pub fn new(s: impl Into<Cow<'static, str>>) -> Self {
        Self(s.into())
    }
}

pub(super) mod py {
    use pyo3::create_exception;

    create_exception!(mystcl, TclError, pyo3::exceptions::Exception);
}

impl From<TclError> for pyo3::PyErr {
    fn from(err: TclError) -> Self {
        py::TclError::py_err(err.0)
    }
}
