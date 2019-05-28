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

impl From<TclError> for std::io::Error {
    fn from(err: TclError) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, err)
    }
}

pub type Result<T, E = TclError> = std::result::Result<T, E>;
