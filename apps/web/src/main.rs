#![no_std]
#![no_main]

mod dns;
mod error;
mod http;
mod net;
mod renderer;
mod util;

#[macro_use]
extern crate alloc;

use crate::{http::HttpClient, renderer::browser::Browser};
use alloc::string::ToString;
use libc_rs::*;

#[unsafe(no_mangle)]
pub fn _start() {
    let _args = parse_args!();

    let client = HttpClient::new();
    let res = client
        .get("example.com".to_string(), 80, "/".to_string())
        .unwrap();
    println!("{:?}", res);

    let browser = Browser::new();
    let page = browser.borrow().current_page();
    let dom_string = page.borrow_mut().receive_response(res);

    for log in dom_string.lines() {
        println!("{}", log);
    }

    unsafe { exit(0) };
}
