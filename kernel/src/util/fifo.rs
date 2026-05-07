use crate::error::{Error, Result};
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
#[repr(C, align(4096))]
struct FifoInner<T: Sized + Copy, const SIZE: usize>([T; SIZE]);

#[derive(Debug)]
pub struct Fifo<T: Sized + Copy, const SIZE: usize> {
    buf: FifoInner<T, SIZE>,
    size: usize,
    read_ptr: AtomicUsize,
    write_ptr: AtomicUsize,
}

impl<T: Sized + Copy, const SIZE: usize> Fifo<T, SIZE> {
    pub const fn new(default: T) -> Self {
        Self {
            buf: FifoInner([default; SIZE]),
            size: SIZE,
            read_ptr: AtomicUsize::new(0),
            write_ptr: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.write_ptr.load(Ordering::Relaxed)
    }

    pub fn is_full(&self) -> bool {
        let read_ptr = self.read_ptr.load(Ordering::Relaxed);
        let write_ptr = self.write_ptr.load(Ordering::Relaxed);
        (write_ptr + 1) % self.size == read_ptr
    }

    pub fn reset_ptr(&self) {
        self.read_ptr.store(0, Ordering::Relaxed);
        self.write_ptr.store(0, Ordering::Relaxed);
    }

    pub fn get_read_write_ptr(&self) -> (usize, usize) {
        (
            self.read_ptr.load(Ordering::Relaxed),
            self.write_ptr.load(Ordering::Relaxed),
        )
    }

    pub fn enqueue(&mut self, value: T) -> Result<()> {
        let read_ptr = self.read_ptr.load(Ordering::Relaxed);
        let write_ptr = self.write_ptr.load(Ordering::Relaxed);
        let next_write_ptr = (write_ptr + 1) % self.size;

        if next_write_ptr == read_ptr {
            return Err(Error::BufferFull.into());
        }

        if self
            .write_ptr
            .compare_exchange(
                write_ptr,
                next_write_ptr,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return Err(Error::Locked.into());
        }

        self.buf.0[write_ptr] = value;

        Ok(())
    }

    pub fn dequeue(&mut self) -> Result<T> {
        let read_ptr = self.read_ptr.load(Ordering::Relaxed);
        let write_ptr = self.write_ptr.load(Ordering::Relaxed);
        let next_read_ptr = (read_ptr + 1) % self.size;

        if read_ptr == write_ptr {
            return Err(Error::BufferEmpty.into());
        }

        if self
            .read_ptr
            .compare_exchange(
                read_ptr,
                next_read_ptr,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            return Err(Error::Locked.into());
        }

        Ok(self.buf.0[read_ptr])
    }

    pub fn get_buf_ref(&self) -> &[T; SIZE] {
        &self.buf.0
    }
}

#[test_case]
fn test_new() {
    let fifo: Fifo<u8, 4> = Fifo::new(0);
    assert_eq!(fifo.len(), 0);
    assert_eq!(fifo.get_read_write_ptr(), (0, 0));
}

#[test_case]
fn test_enqueue_dequeue() {
    let mut fifo: Fifo<u8, 4> = Fifo::new(0);
    assert!(fifo.enqueue(1).is_ok());
    assert!(fifo.enqueue(2).is_ok());
    assert!(fifo.enqueue(3).is_ok());
    assert!(fifo.enqueue(4).is_err());

    assert_eq!(fifo.dequeue(), Ok(1));
    assert_eq!(fifo.dequeue(), Ok(2));
    assert_eq!(fifo.dequeue(), Ok(3));
    assert!(fifo.dequeue().is_err());
}

#[test_case]
fn test_reset() {
    let mut fifo: Fifo<u8, 4> = Fifo::new(0);
    fifo.enqueue(1).unwrap();
    fifo.enqueue(2).unwrap();
    fifo.enqueue(3).unwrap();
    fifo.reset_ptr();

    assert_eq!(fifo.get_read_write_ptr(), (0, 0));
    assert!(fifo.dequeue().is_err());
}
