#![no_std]
#![no_main]

mod dns;
mod error;
mod http;
mod net;

extern crate alloc;

use crate::http::HttpClient;
use alloc::string::ToString;
use libc_rs::*;

#[unsafe(no_mangle)]
pub fn _start() {
    let _args = parse_args!();

    let client = HttpClient::new();
    let res = client.get("example.com".to_string(), 80, "/".to_string());
    println!("{:?}", res);
    unsafe { exit(0) };
}
