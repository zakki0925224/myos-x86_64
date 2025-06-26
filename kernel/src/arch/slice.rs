use crate::error::{Error, Result};
use core::slice;

pub unsafe trait Sliceable: Sized + Clone + Copy {
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const Self as *const u8, size_of::<Self>()) }
    }

    fn copy_from_slice(data: &[u8]) -> Result<Self> {
        if size_of::<Self>() > data.len() {
            Err(Error::Failed("data is too short"))
        } else {
            Ok(unsafe { *(data.as_ptr() as *const Self) })
        }
    }
}
