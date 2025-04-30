use super::{
    bitmap::{self, MemoryFrameInfo},
    paging::PAGE_SIZE,
};
use crate::error::{Error, Result};
use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
    slice,
};

#[derive(Debug)]
pub struct FrameVec<T: Copy> {
    mem_frame_info: MemoryFrameInfo,
    len: usize,
    capacity: usize,
    _marker: PhantomData<T>,
}

impl<T: Copy> Drop for FrameVec<T> {
    fn drop(&mut self) {
        bitmap::dealloc_mem_frame(self.mem_frame_info).unwrap();
    }
}

impl<T: Copy> Index<usize> for FrameVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!("Index out of bounds");
        }

        let buf = self.buf(index).unwrap();
        unsafe { &*buf }
    }
}

impl<T: Copy> IndexMut<usize> for FrameVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            panic!("Index out of bounds");
        }

        let buf_mut = self.buf_mut(index).unwrap();
        unsafe { &mut *buf_mut }
    }
}

impl<T: Copy> Deref for FrameVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        let buf = self.buf(0).unwrap();
        unsafe { slice::from_raw_parts(buf, self.len) }
    }
}

impl<T: Copy> DerefMut for FrameVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let buf_mut = self.buf_mut(0).unwrap();
        unsafe { slice::from_raw_parts_mut(buf_mut, self.len) }
    }
}

impl<T: Copy> FrameVec<T> {
    pub fn new(capacity: usize) -> Result<Self> {
        let alloc_len = (size_of::<T>() * capacity).div_ceil(PAGE_SIZE);
        let mem_frame_info = bitmap::alloc_mem_frame(alloc_len)?;

        let vec = Self {
            mem_frame_info,
            len: 0,
            capacity,
            _marker: PhantomData,
        };
        Ok(vec)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn fill(&mut self, value: T) -> Result<()> {
        for i in 0..self.capacity {
            let buf_mut = self.buf_mut(i)?;
            unsafe {
                *buf_mut = value;
            }
        }

        Ok(())
    }

    pub fn push(&mut self, value: T) -> Result<()> {
        let buf_mut = self.buf_mut(self.len)?;
        unsafe {
            *buf_mut = value;
        }

        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Option<T>> {
        if self.len == 0 {
            return Ok(None);
        }

        let buf_mut = self.buf_mut(self.len)?;
        let value = unsafe { *buf_mut };
        self.len -= 1;

        Ok(Some(value))
    }

    fn buf_mut(&mut self, index: usize) -> Result<*mut T> {
        if index >= self.capacity {
            return Err(Error::IndexOutOfBoundsError(index));
        }

        let buf_ptr_mut = self
            .mem_frame_info
            .frame_start_virt_addr()?
            .offset(size_of::<T>() * index)
            .as_ptr_mut();
        Ok(buf_ptr_mut)
    }

    fn buf(&self, index: usize) -> Result<*const T> {
        if index >= self.capacity {
            return Err(Error::IndexOutOfBoundsError(index));
        }

        let buf_ptr = self
            .mem_frame_info
            .frame_start_virt_addr()?
            .offset(size_of::<T>() * index)
            .as_ptr();
        Ok(buf_ptr)
    }
}
