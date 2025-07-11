#![no_std]
#![no_main]

use libc_rs::*;

#[no_mangle]
pub unsafe fn _start() {
    let args = parse_args!();

    println!("Hello world!");
    println!("{:?}", args);
    exit(0);
}
