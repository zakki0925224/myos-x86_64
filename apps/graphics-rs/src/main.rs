#![no_std]
#![no_main]

use core::convert::Infallible;
use embedded_graphics::{
    mono_font::{iso_8859_15::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::*,
    text::Text,
};
use libc_rs::*;

const WIDTH: usize = 400;
const HEIGHT: usize = 300;

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

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
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
    // create window
    let title = "graphics-rs\0";
    let cdesc_window = create_component_window(
        title.as_ptr() as *const _,
        100,
        100,
        WIDTH + 10,
        HEIGHT + 50,
    );
    if cdesc_window.is_null() {
        let msg = "Failed to create component window\n\0";
        printf(msg.as_ptr() as *const _);
        exit(-1);
    }

    // initialize framebuffer
    let fb = malloc((WIDTH * HEIGHT * 4) as u64);
    if fb.is_null() {
        let msg = "Failed to allocate framebuffer memory\n\0";
        printf(msg.as_ptr() as *const _);
        exit(-1);
    }

    // create image to window
    let cdesc_image =
        create_component_image(cdesc_window, WIDTH, HEIGHT, PIXEL_FORMAT_BGRA as u8, fb);
    if cdesc_image.is_null() {
        let msg = "Failed to create component image\n\0";
        printf(msg.as_ptr() as *const _);
        exit(-1);
    }

    let mut eg_fb = Framebuffer {
        fb: fb as *mut u8,
        width: WIDTH,
        height: HEIGHT,
    };

    Rectangle::new(Point::new(50, 50), Size::new(100, 100))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::RED))
        .draw(&mut eg_fb)
        .unwrap();

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);
    Text::new(
        "Hello, graphics-rs with\nembedded-graphics!",
        Point::new(60, 60),
        text_style,
    )
    .draw(&mut eg_fb)
    .unwrap();

    loop {}
}
