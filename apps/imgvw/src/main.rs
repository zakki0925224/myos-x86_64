#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*, primitives::*};
use libc_rs::*;
use tinygif::Gif;

const WIDTH: usize = 450;
const HEIGHT: usize = 400;

struct Framebuffer {
    fb: *mut u8,
    width: usize,
    height: usize,
}

impl Dimensions for Framebuffer {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(
            Point::zero(),
            Size::new(self.width as u32, self.height as u32),
        )
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> core::result::Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let x = coord.x as usize;
            let y = coord.y as usize;
            if x < self.width && y < self.height {
                let offset = (y * self.width + x) * 4;
                unsafe {
                    let pixel_ptr = self.fb.add(offset);
                    *pixel_ptr = color.b();
                    *pixel_ptr.add(1) = color.g();
                    *pixel_ptr.add(2) = color.r();
                    *pixel_ptr.add(3) = 0xff; // Alpha channel
                }
            }
        }

        Ok(())
    }
}

#[no_mangle]
pub unsafe fn _start() {
    let args = parse_args!();

    if args.len() < 2 {
        println!("Usage: imgvw <IMAGE FILE PATH>");
        exit(-1);
    }

    // open file
    let file = match File::open(args[1]) {
        Ok(f) => f,
        Err(err) => {
            println!("Failed to open the file: {:?}", err);
            exit(-1);
        }
    };

    // read file
    let mut buf: Vec<u8> = vec![0; file.size()];
    if let Err(err) = file.read(buf.as_mut_slice()) {
        println!("Failed to read the file: {:?}", err);
        exit(-1);
    }

    let gif = match Gif::<Rgb888>::from_slice(buf.as_slice()) {
        Ok(gif) => gif,
        Err(err) => {
            println!("Failed to parse GIF(RGB888): {:?}", err);
            exit(-1);
        }
    };

    // create window
    let title = format!("{} - imgvw\0", args[1]);
    let cdesc_window = create_component_window(
        title.as_ptr() as *const _,
        100,
        100,
        WIDTH + 10,
        HEIGHT + 50,
    );
    if cdesc_window.is_null() {
        println!("Failed to create component window");
        exit(-1);
    }

    // initialize framebuffer
    let fb = malloc((WIDTH * HEIGHT * 4) as u64);
    if fb.is_null() {
        println!("Failed to allocate framebuffer memory");
        exit(-1);
    }

    // create image to window
    let cdesc_image =
        create_component_image(cdesc_window, WIDTH, HEIGHT, PIXEL_FORMAT_BGRA as u8, fb);
    if cdesc_image.is_null() {
        println!("Failed to create component image");
        exit(-1);
    }

    let mut eg_fb = Framebuffer {
        fb: fb as *mut u8,
        width: WIDTH,
        height: HEIGHT,
    };

    loop {
        for frame in gif.frames() {
            frame.draw(&mut eg_fb).unwrap();
        }
    }
}
