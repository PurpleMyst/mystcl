use pyo3::{exceptions::Exception, create_exception};

create_exception!(mystcl, TclError, Exception);
