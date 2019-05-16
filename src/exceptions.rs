use pyo3::{create_exception, exceptions::Exception};

create_exception!(mystcl, TclError, Exception);
