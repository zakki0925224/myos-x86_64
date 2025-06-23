use alloc::boxed::Box;
use core::{mem::ManuallyDrop, pin::Pin};

#[derive(Debug)]
pub struct Mmio<T: Sized> {
    inner: ManuallyDrop<Pin<Box<T>>>,
}

impl<T> AsRef<T> for Mmio<T> {
    fn as_ref(&self) -> &T {
        self.inner.as_ref().get_ref()
    }
}

impl <T: Unpin> AsMut<T> for Mmio<T> {
    fn as_mut(&mut self) -> &mut T {
        self.inner.as_mut().get_mut()
    }
}

impl<T: Sized + Unpin> Mmio<T> {
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self {
            inner: ManuallyDrop::new(Box::into_pin(Box::from_raw(ptr))),
        }
    }

    pub unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        self.inner.as_mut().get_unchecked_mut()
    }
}
