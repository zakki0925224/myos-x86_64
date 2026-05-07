use crate::error::{Error, Result};
use core::slice;

pub unsafe trait Sliceable: Sized + Clone + Copy {
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const Self as *const u8, size_of::<Self>()) }
    }

    fn copy_from_slice(data: &[u8]) -> Result<Self> {
        let len = size_of::<Self>();
        if len > data.len() {
            Err(Error::InvalidBufferSize {
                required: len,
                actual: data.len(),
            }
            .into())
        } else {
            Ok(unsafe { *(data.as_ptr() as *const Self) })
        }
    }
}
