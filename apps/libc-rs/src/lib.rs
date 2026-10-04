#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(suspicious_runtime_symbol_definitions)]

#[cfg(not(feature = "kernel"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "kernel"))]
use alloc::{ffi::CString, vec::Vec};
#[cfg(not(feature = "kernel"))]
use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    str::FromStr,
};
#[cfg(not(feature = "kernel"))]
use linked_list_allocator::LockedHeap;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// result/error
#[cfg(not(feature = "kernel"))]
#[derive(Debug, Clone, PartialEq)]
pub enum LibcError {
    FopenFailed,
    FreadFailed,
    OpenFailed,
    ReadFailed,
    Failed,
}

#[cfg(not(feature = "kernel"))]
pub type Result<T> = core::result::Result<T, LibcError>;

// heap
#[cfg(not(feature = "kernel"))]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(not(feature = "kernel"))]
#[doc(hidden)]
pub fn _init_heap() {
    let heap_size = 1024 * 1024;
    let heap = unsafe { malloc(heap_size as u64) as *mut u8 };
    unsafe {
        ALLOCATOR.lock().init(heap, heap_size);
    }
}

// panic
#[cfg(not(feature = "kernel"))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:?}", info.message());
    println!("{:?}", info.location());

    unsafe {
        exit(-1);
    }
}

// parse args macro
#[cfg(not(feature = "kernel"))]
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

#[cfg(not(feature = "kernel"))]
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
#[cfg(not(feature = "kernel"))]
struct Writer;

#[cfg(not(feature = "kernel"))]
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            printf(format!("{}\0", s).as_ptr() as *const _);
        }

        Ok(())
    }
}

#[cfg(not(feature = "kernel"))]
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    Writer.write_fmt(args).unwrap();
}

#[cfg(not(feature = "kernel"))]
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[cfg(not(feature = "kernel"))]
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// file
#[cfg(not(feature = "kernel"))]
#[repr(C)]
pub struct File {
    ptr: *mut FILE,
}

#[cfg(not(feature = "kernel"))]
impl Drop for File {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                fclose(self.ptr);
            }
        }
    }
}

#[cfg(not(feature = "kernel"))]
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

// fd
#[cfg(not(feature = "kernel"))]
pub struct Fd(i32);

#[cfg(not(feature = "kernel"))]
impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            sys_close(self.0);
        }
    }
}

#[cfg(not(feature = "kernel"))]
impl Fd {
    pub fn open(path: &str, flags: u32) -> Result<Self> {
        let path = CString::from_str(path).unwrap();
        let fd = unsafe { sys_open(path.as_ptr(), flags as i32) };
        if fd < 0 {
            return Err(LibcError::OpenFailed);
        }
        Ok(Self(fd))
    }

    pub fn raw(&self) -> i32 {
        self.0
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let len = unsafe { sys_read(self.0, buf.as_mut_ptr() as *mut _, buf.len() as _) };
        if len < 0 {
            return Err(LibcError::ReadFailed);
        }
        Ok(len as usize)
    }

    pub fn read_events<T: Copy>(&self, buf: &mut [T]) -> Result<usize> {
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                buf.as_mut_ptr() as *mut u8,
                buf.len() * core::mem::size_of::<T>(),
            )
        };
        Ok(self.read(bytes)? / core::mem::size_of::<T>())
    }

    pub fn poll_in(&self) -> pollfd {
        pollfd {
            fd: self.0,
            events: POLLIN as i16,
            revents: 0,
        }
    }
}

// poll
#[cfg(not(feature = "kernel"))]
pub fn poll(fds: &mut [pollfd], timeout_ms: i64) -> Result<usize> {
    let n = unsafe { sys_poll(fds.as_mut_ptr(), fds.len() as _, timeout_ms) };
    if n < 0 {
        return Err(LibcError::Failed);
    }
    Ok(n as usize)
}

// screen
#[cfg(not(feature = "kernel"))]
pub fn screen_size() -> Result<(usize, usize)> {
    let mut width = 0;
    let mut height = 0;
    if unsafe { get_screen_size(&mut width, &mut height) } < 0 {
        return Err(LibcError::Failed);
    }
    Ok((width as usize, height as usize))
}

// layer
#[cfg(not(feature = "kernel"))]
pub struct Layer(i32);

#[cfg(not(feature = "kernel"))]
impl Drop for Layer {
    fn drop(&mut self) {
        unsafe {
            remove_layer(self.0);
        }
    }
}

#[cfg(not(feature = "kernel"))]
impl Layer {
    pub fn create(
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        pixels: &[u32],
        always_on_top: bool,
    ) -> Result<Self> {
        assert_eq!(pixels.len(), width * height);

        let flags = if always_on_top {
            LAYER_FLAG_ALWAYS_ON_TOP
        } else {
            0
        };
        let id = unsafe {
            create_layer(
                x as _,
                y as _,
                width as _,
                height as _,
                pixels.as_ptr() as *const _,
                flags,
            )
        };
        if id < 0 {
            return Err(LibcError::Failed);
        }
        Ok(Self(id))
    }

    pub fn move_to(&self, x: usize, y: usize) -> Result<()> {
        if unsafe { move_layer(self.0, x as _, y as _) } < 0 {
            return Err(LibcError::Failed);
        }
        Ok(())
    }
}
