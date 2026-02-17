#![no_std]
#![no_main]

mod constsnt;
mod display_item;
mod dns;
mod error;
mod http;
mod net;
mod renderer;
mod ui;

#[macro_use]
extern crate alloc;

use crate::constsnt::*;
use crate::http::HttpClient;
use crate::renderer::browser::Browser;
use crate::ui::{Framebuffer, paint_display_items};
use libc_rs::*;

fn parse_url(raw: &str) -> (&str, u16, &str) {
    let s = raw.strip_prefix("http://").unwrap_or(raw);

    let (host_port, path) = match s.find('/') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, "/"),
    };

    let (host, port) = match host_port.rfind(':') {
        Some(i) => (
            &host_port[..i],
            host_port[i + 1..].parse::<u16>().unwrap_or(80),
        ),
        None => (host_port, 80),
    };

    (host, port, path)
}

#[unsafe(no_mangle)]
pub fn _start() {
    let args = parse_args!();
    let raw_url = if args.len() < 2 {
        "example.com"
    } else {
        args[1]
    };

    let (host, port, path) = parse_url(raw_url);
    println!("Connecting to {}:{}{}", host, port, path);

    let client = HttpClient::new();
    let res = client.get(host, port, path).unwrap();

    let browser = Browser::new();
    let page = browser.borrow().current_page();
    page.borrow_mut().receive_response(res);

    let display_items = browser.borrow().current_page().borrow().display_items();
    let page_title = browser.borrow().current_page().borrow().title();

    let content_w = CONTENT_AREA_WIDTH as usize;
    let content_h = CONTENT_AREA_HEIGHT as usize;

    let title = if page_title.is_empty() {
        format!("{}\0", raw_url)
    } else {
        format!("{} - {}\0", page_title, raw_url)
    };
    let cdesc_window = unsafe {
        create_component_window(
            title.as_ptr() as *const _,
            50,
            50,
            content_w + WINDOW_PADDING as usize * 2,
            content_h + WINDOW_PADDING as usize * 2 + TITLE_BAR_HEIGHT as usize,
        )
    };
    if cdesc_window.is_null() {
        println!("Failed to create window");
        unsafe { exit(-1) };
    }

    let fb = unsafe { malloc((content_w * content_h * 4) as u64) };
    if fb.is_null() {
        println!("Failed to allocate framebuffer");
        unsafe { exit(-1) };
    }

    let cdesc_image = unsafe {
        create_component_image(
            cdesc_window,
            content_w,
            content_h,
            PIXEL_FORMAT_BGRA as u8,
            fb,
        )
    };
    if cdesc_image.is_null() {
        println!("Failed to create image component");
        unsafe { exit(-1) };
    }

    let mut eg_fb = Framebuffer::new(fb as *mut u8, content_w, content_h);
    paint_display_items(&mut eg_fb, &display_items);

    loop {
        print!(""); // yield
    }
}
