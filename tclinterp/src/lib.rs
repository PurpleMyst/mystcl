#![deny(unused_imports, unused_must_use)]
#![allow(clippy::identity_conversion)] // Because clippy complains about pyo3.

// FIXME: Use a custom-built type instead of `CStr` to handle strings containing NUL bytes.

mod exceptions;
mod postoffice;
mod tclinterp;
mod tclobj;
mod tclsocket;
mod wrappers;

pub use crate::exceptions::TclError;
pub use crate::tclinterp::TclInterp;
pub use crate::tclobj::{TclObj, ToTclObj};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert!(TclInterp::new().is_ok());
    }

    #[test]
    fn test_call() {
        assert_eq!(
            TclInterp::new()
                .unwrap()
                .call(&["format", "%s", "hello, world"])
                .unwrap(),
            "hello, world"
        );
    }

    #[test]
    fn test_delete() {
        let mut interp = TclInterp::new().unwrap();
        assert!(!interp.deleted());
        interp.delete().unwrap();
        assert!(interp.deleted());

        let err = interp.call(&["format", "%s", "test123"]).unwrap_err();
        assert_eq!(err.0, "Tried to use interpreter after deletion");
    }

    #[test]
    fn test_eval() {
        assert_eq!(
            TclInterp::new()
                .unwrap()
                .eval("format %s {42}".to_owned())
                .unwrap(),
            "42"
        );
    }

    #[test]
    fn test_splitlist() {
        let mut interp = TclInterp::new().unwrap();

        let l1 = interp.call(&["list", "a", "b", "c and d"]).unwrap();

        let mut l1_parts = interp.splitlist(&l1 as &str).unwrap();
        l1_parts.insert(0, "list".to_owned());

        let l2 = interp.call(l1_parts.iter().map(|s| s as &str)).unwrap();

        assert_eq!(l1, l2);
    }
}
