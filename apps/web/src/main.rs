#![no_std]
#![no_main]

mod constsnt;
mod display_item;
mod dns;
mod error;
mod http;
mod net;
mod renderer;

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
    page.borrow_mut().receive_response(res);

    for item in browser.borrow().current_page().borrow().display_items() {
        println!("{:?}", item);
    }

    unsafe { exit(0) };
}
