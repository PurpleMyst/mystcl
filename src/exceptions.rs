use std::{borrow::Cow, error, fmt};

/// Represents an error returned from the Tcl interpreter.
///
/// This is usually just the error returned from Tcl itself.
#[derive(Debug, Clone)]
pub struct TclError(pub Cow<'static, str>);

impl TclError {
    pub fn new(s: impl Into<Cow<'static, str>>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for TclError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl error::Error for TclError {}

pub(super) mod py {
    use pyo3::create_exception;

    create_exception!(mystcl, TclError, pyo3::exceptions::Exception);
}

impl From<TclError> for pyo3::PyErr {
    fn from(err: TclError) -> Self {
        py::TclError::py_err(err.0)
    }
}
