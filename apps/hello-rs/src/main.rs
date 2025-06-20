#![no_std]
#![no_main]

use libc_rs::*;

#[no_mangle]
pub unsafe fn _start() {
    init_heap();
    println!("Hello world!");
    exit(0);
}
