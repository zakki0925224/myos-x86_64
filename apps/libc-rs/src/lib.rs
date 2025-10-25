#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[cfg(not(feature = "for-kernel-stubs"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "for-kernel-stubs"))]
use alloc::{ffi::CString, vec::Vec};
#[cfg(not(feature = "for-kernel-stubs"))]
use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    str::FromStr,
};
#[cfg(not(feature = "for-kernel-stubs"))]
use linked_list_allocator::LockedHeap;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// result/error
#[cfg(not(feature = "for-kernel-stubs"))]
#[derive(Debug, Clone, PartialEq)]
pub enum LibcError {
    FopenFailed,
    FreadFailed,
}

#[cfg(not(feature = "for-kernel-stubs"))]
pub type Result<T> = core::result::Result<T, LibcError>;

// heap
#[cfg(not(feature = "for-kernel-stubs"))]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(not(feature = "for-kernel-stubs"))]
#[doc(hidden)]
pub fn _init_heap() {
    let heap_size = 1024 * 1024;
    let heap = unsafe { malloc(heap_size as u64) as *mut u8 };
    unsafe {
        ALLOCATOR.lock().init(heap, heap_size);
    }
}

// panic
#[cfg(not(feature = "for-kernel-stubs"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:?}", info.message());
    println!("{:?}", info.location());

    unsafe {
        exit(-1);
    }
}

// parse args macro
#[cfg(not(feature = "for-kernel-stubs"))]
#[doc(hidden)]
pub unsafe fn _parse_args(argc: usize, argv: *const *const u8) -> Vec<&'static str> {
    let mut args = Vec::new();
    for i in 0..argc {
        let ptr = *argv.add(i);
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let slice = core::slice::from_raw_parts(ptr, len);
        let s = match str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => "",
        };
        args.push(s);
    }

    args
}

#[cfg(not(feature = "for-kernel-stubs"))]
#[macro_export]
macro_rules! parse_args {
    () => {{
        use core::arch::asm;

        let argc: usize;
        let argv: *const *const u8;
        unsafe {
            asm!("mov {}, rdi", out(reg) argc, options(nomem, nostack));
            asm!("mov {}, rsi", out(reg) argv, options(nomem, nostack));
        }

        $crate::_init_heap();
        let args = unsafe { $crate::_parse_args(argc, argv) };
        args
    }};
}

// print macros
#[cfg(not(feature = "for-kernel-stubs"))]
struct Writer;

#[cfg(not(feature = "for-kernel-stubs"))]
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            printf(format!("{}\0", s).as_ptr() as *const _);
        }

        Ok(())
    }
}

#[cfg(not(feature = "for-kernel-stubs"))]
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    Writer.write_fmt(args).unwrap();
}

#[cfg(not(feature = "for-kernel-stubs"))]
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[cfg(not(feature = "for-kernel-stubs"))]
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// file
#[cfg(not(feature = "for-kernel-stubs"))]
#[repr(C)]
pub struct File {
    ptr: *mut FILE,
}

#[cfg(not(feature = "for-kernel-stubs"))]
impl Drop for File {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                fclose(self.ptr);
            }
        }
    }
}

#[cfg(not(feature = "for-kernel-stubs"))]
impl File {
    fn call_fopen(path: &str, mode: char) -> Result<Self> {
        let path_cstr = CString::from_str(path).unwrap();
        let path = path_cstr.as_bytes_with_nul();

        let mut buf = [0; 4];
        let encoded = mode.encode_utf8(&mut buf);
        let mode_cstr = CString::new(encoded.as_bytes()).unwrap();
        let mode = mode_cstr.as_bytes_with_nul();

        let file_ptr = unsafe { fopen(path.as_ptr() as *const i8, mode.as_ptr() as *const i8) };

        if file_ptr.is_null() {
            return Err(LibcError::FopenFailed);
        }

        Ok(Self { ptr: file_ptr })
    }

    fn call_fread(&self, buf: &mut [u8]) -> Result<()> {
        match unsafe { fread(buf.as_mut_ptr() as *mut _, 1, buf.len() as u64, self.ptr) } {
            0 => Err(LibcError::FreadFailed),
            _ => Ok(()),
        }
    }

    pub fn size(&self) -> usize {
        unsafe { (*(*self.ptr).stat).size }
    }

    pub fn open(path: &str) -> Result<Self> {
        Self::call_fopen(path, 'r')
    }

    pub fn create(path: &str) -> Result<Self> {
        Self::call_fopen(path, 'w')
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<()> {
        self.call_fread(buf)
    }
}
