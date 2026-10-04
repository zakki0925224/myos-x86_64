use crate::error::{Error, Result};
use alloc::{collections::VecDeque, vec::Vec};
use core::cmp::min;

#[derive(Debug, Default)]
pub struct PipeBuffer {
    buf: VecDeque<u8>,
    write_closed: bool,
}

impl PipeBuffer {
    pub fn read(&mut self, max_len: usize) -> Result<Vec<u8>> {
        if self.buf.is_empty() {
            return if self.write_closed {
                Ok(Vec::new())
            } else {
                Err(Error::BufferEmpty.into())
            };
        }

        let len = min(max_len, self.buf.len());
        Ok(self.buf.drain(..len).collect())
    }

    pub fn write(&mut self, data: &[u8]) {
        self.buf.extend(data);
    }

    pub fn close_write(&mut self) {
        self.write_closed = true;
    }

    pub fn reopen_write(&mut self) {
        self.write_closed = false;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn readable(&self) -> bool {
        !self.buf.is_empty() || self.write_closed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeEnd {
    Read,
    Write,
}
