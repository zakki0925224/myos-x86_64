use core::{
    mem::MaybeUninit,
    ptr::{read_volatile, write_volatile},
};

#[repr(transparent)]
#[derive(Debug)]
pub struct Volatile<T>(T);

impl<T: Default> Default for Volatile<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<T: Clone> Clone for Volatile<T> {
    fn clone(&self) -> Self {
        let this = MaybeUninit::uninit();
        let mut this: Self = unsafe { this.assume_init() };
        this.write(self.read());
        this
    }
}

impl<T> Volatile<T> {
    pub fn read(&self) -> T {
        unsafe { read_volatile(&self.0) }
    }

    pub fn write(&mut self, value: T) {
        unsafe { write_volatile(&mut self.0, value) }
    }
}

#[test_case]
fn read_write() {
    let mut v: Volatile<u16> = Volatile::default();
    assert_eq!(v.read(), 0);
    v.write(0x1234);
    assert_eq!(v.read(), 0x1234);
    v.write(0x5678);
    assert_eq!(v.read(), 0x5678);
}
