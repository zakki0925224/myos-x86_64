#![no_std]
#![no_main]

mod constsnt;
mod dns;
mod error;
mod http;
mod net;
mod renderer;
mod util;

#[macro_use]
extern crate alloc;

use crate::{http::HttpClient, renderer::browser::Browser};
use libc_rs::*;

#[unsafe(no_mangle)]
pub fn _start() {
    let args = parse_args!();
    let host = if args.len() < 2 {
        "example.com"
    } else {
        args[1]
    };

    let client = HttpClient::new();
    let res = client.get(host, 80, "/").unwrap();
    println!("{:?}", res);

    let browser = Browser::new();
    let page = browser.borrow().current_page();
    let dom_string = page.borrow_mut().receive_response(res);

    for log in dom_string.lines() {
        println!("{}", log);
    }

    unsafe { exit(0) };
}
