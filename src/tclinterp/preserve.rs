use std::ptr::NonNull;

#[cfg(test)]
use std::os::raw::c_char;

#[derive(Debug)]
pub struct Preserve<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> Preserve<T> {
    pub fn new(ptr: NonNull<T>) -> Self {
        let mut inst = Self(ptr);
        inst.preserve();
        inst
    }

    fn client_data(&self) -> tcl_sys::ClientData {
        self.0.as_ptr() as tcl_sys::ClientData
    }

    fn preserve(&mut self) {
        unsafe { tcl_sys::Tcl_Preserve(self.client_data()) }
    }

    fn release(&mut self) {
        unsafe { tcl_sys::Tcl_Release(self.client_data()) }
    }

    #[cfg(test)]
    unsafe fn eventually_free(&self, free_proc: extern "C" fn(*mut c_char) -> ()) {
        tcl_sys::Tcl_EventuallyFree(self.client_data(), Some(free_proc));
    }

    pub fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: ?Sized> Drop for Preserve<T> {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{mem, ptr::NonNull};

    #[test]
    fn test_preserve() {
        static mut COUNTER: usize = 0;

        let data = Preserve::new(NonNull::new(Box::into_raw(Box::new(()))).unwrap());

        extern "C" fn free_proc(_: *mut c_char) {
            unsafe {
                COUNTER += 1;
            }
        }

        unsafe {
            data.eventually_free(free_proc);
            assert_eq!(COUNTER, 0);
            mem::drop(data);
            assert_eq!(COUNTER, 1);
        }
    }
}
