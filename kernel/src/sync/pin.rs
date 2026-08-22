use crate::error::{Error, Result};
use core::{pin::Pin, slice};

/// # Safety
///
/// Implementors must be plain-old-data: the default methods reinterpret `Self`
/// as `size_of::<Self>()` bytes, so the type must have no padding that would be
/// exposed, no interior pointers, and every bit pattern must be valid.
pub unsafe trait IntoPinnedMutableSlice: Sized + Copy + Clone {
    fn as_mut_slice(self: Pin<&mut Self>) -> Pin<&mut [u8]> {
        Pin::new(unsafe {
            slice::from_raw_parts_mut(
                self.get_unchecked_mut() as *mut Self as *mut u8,
                size_of::<Self>(),
            )
        })
    }

    fn as_mut_slice_sized(self: Pin<&mut Self>, size: usize) -> Result<Pin<&mut [u8]>> {
        if size > size_of::<Self>() {
            Err(Error::InvalidBufferSize {
                required: size_of::<Self>(),
                actual: size,
            }
            .into())
        } else {
            Ok(Pin::new(unsafe {
                slice::from_raw_parts_mut(self.get_unchecked_mut() as *mut Self as *mut u8, size)
            }))
        }
    }
}
