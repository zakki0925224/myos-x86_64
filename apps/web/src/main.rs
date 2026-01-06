#![no_std]
#![no_main]

mod dns;
mod error;
mod http;
mod net;

extern crate alloc;

use libc_rs::*;

use crate::dns::{DnsClient, QEMU_DNS};

#[unsafe(no_mangle)]
pub fn _start() {
    let _args = parse_args!();

    let client = DnsClient::new(QEMU_DNS);
    let ip = client.resolve("google.com").unwrap();

    println!("ip: {:?}", ip);
    unsafe { exit(0) };
}
