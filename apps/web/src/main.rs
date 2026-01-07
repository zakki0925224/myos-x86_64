#![no_std]
#![no_main]

mod dns;
mod dom;
mod error;
mod html;
mod http;
mod net;

#[macro_use]
extern crate alloc;

use crate::http::HttpClient;
use alloc::string::ToString;
use libc_rs::*;

#[unsafe(no_mangle)]
pub fn _start() {
    let _args = parse_args!();

    let client = HttpClient::new();
    let res = client.get("localhost".to_string(), 8888, "/".to_string());
    println!("{:?}", res);
    unsafe { exit(0) };
}
