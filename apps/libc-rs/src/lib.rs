#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[macro_use]
extern crate alloc;

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};
use linked_list_allocator::LockedHeap;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// heap
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_heap() {
    let heap_size = 1024 * 1024;
    let heap = unsafe { malloc(heap_size as u64) as *mut u8 };
    unsafe {
        ALLOCATOR.lock().init(heap, heap_size);
    }
}

// panic
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:?}", info.message());
    println!("{:?}", info.location());

    unsafe {
        exit(-1);
    }
}

// print macros
struct Writer;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            printf(format!("{}\0", s).as_ptr() as *const _);
        }

        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    Writer.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
