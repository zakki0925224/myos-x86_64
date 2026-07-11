use crate::arch::{
    x86_64::paging::{self, PageWriteThroughLevel, ReadWrite, PAGE_SIZE},
    VirtualAddress,
};
use alloc::boxed::Box;
use core::{
    marker::PhantomPinned,
    mem::{ManuallyDrop, MaybeUninit},
    pin::Pin,
};

#[derive(Debug)]
pub struct Mmio<T: Sized> {
    inner: ManuallyDrop<Pin<Box<T>>>,
}

impl<T> AsRef<T> for Mmio<T> {
    fn as_ref(&self) -> &T {
        self.inner.as_ref().get_ref()
    }
}

impl<T: Unpin> AsMut<T> for Mmio<T> {
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

#[repr(align(4096))]
pub struct IoBoxInner<T: Sized> {
    data: T,
    _pinned: PhantomPinned,
}

impl<T: Sized> IoBoxInner<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            _pinned: PhantomPinned,
        }
    }
}

pub struct IoBox<T: Sized> {
    inner: Pin<Box<IoBoxInner<T>>>,
}

impl<T> AsRef<T> for IoBox<T> {
    fn as_ref(&self) -> &T {
        &self.inner.as_ref().get_ref().data
    }
}

impl<T: Default> Default for IoBox<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Sized> IoBox<T> {
    pub fn new() -> Self {
        let inner = Box::pin(IoBoxInner::new(unsafe {
            MaybeUninit::<T>::zeroed().assume_init()
        }));

        let this = Self { inner };

        // disable cache
        let start: VirtualAddress = (this.as_ref() as *const T as u64).into();
        let end = start.offset(size_of::<T>().div_ceil(PAGE_SIZE) * PAGE_SIZE);

        unsafe {
            paging::kernel_map(
                start,
                end,
                ReadWrite::Write,
                PageWriteThroughLevel::WriteThrough,
                true, // disable cache
            )
            .unwrap();
        };

        this
    }

    pub unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        &mut self.inner.as_mut().get_unchecked_mut().data
    }
}
