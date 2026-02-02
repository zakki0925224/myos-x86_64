use crate::error::Result;
use core::{
    cell::SyncUnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Mutex<T> {
    value: SyncUnsafeCell<T>,
    locked: AtomicBool,
}

impl<T: Sized> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: SyncUnsafeCell::new(value),
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Result<MutexGuard<T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Ok(unsafe { MutexGuard::new(self, &self.value) });
        }

        Err("Mutex is already locked".into())
    }

    pub unsafe fn get_force_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    pub fn spin_lock(&self) -> MutexGuard<T> {
        loop {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return unsafe { MutexGuard::new(self, &self.value) };
            }

            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
    }
}

unsafe impl<T> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    value: &'a mut T,
}

impl<'a, T> MutexGuard<'a, T> {
    unsafe fn new(mutex: &'a Mutex<T>, value: &SyncUnsafeCell<T>) -> Self {
        Self {
            mutex,
            value: &mut *value.get(),
        }
    }
}

unsafe impl<'a, T> Sync for MutexGuard<'a, T> {}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

#[test_case]
fn test_lock_unlock() {
    let mutex = Mutex::new(0);

    {
        let mut guard = mutex.try_lock().unwrap();
        *guard += 1;
        assert_eq!(*guard, 1);
    }

    {
        let guard = mutex.try_lock().unwrap();
        assert_eq!(*guard, 1);
    }
}

#[test_case]
fn test_unlock_force() {
    let mut mutex = Mutex::new(0);

    unsafe {
        let guard = mutex.get_force_mut();
        *guard += 1;
    }

    let guard = mutex.try_lock().unwrap();
    assert_eq!(*guard, 1);
}
