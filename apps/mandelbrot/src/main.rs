#![no_std]
#![no_main]

use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*, primitives::*};
use libc_rs::*;

const WIDTH: usize = 450;
const HEIGHT: usize = 400;

const SCALE: i32 = 1 << 16; // 16.16 fixed-point
const MAX_ITER: u32 = 100;

const MIN_RE: i32 = (-2200 * SCALE) / 1000; // -2.2
const MAX_RE: i32 = (800 * SCALE) / 1000; // 0.8
const MIN_IM: i32 = (-1200 * SCALE) / 1000; // -1.2
const MAX_IM: i32 = (1200 * SCALE) / 1000; // +1.2

fn map_to_real(x: usize, width: usize) -> i32 {
    let min = MIN_RE as i64;
    let max = MAX_RE as i64;
    let range = max - min;
    let x = x as i64;
    let width = width as i64;
    let scaled = min + (range * x / (width - 1));
    scaled as i32
}

fn map_to_imag(y: usize, height: usize) -> i32 {
    let min = MAX_IM as i64;
    let max = MIN_IM as i64;
    let range = max - min;
    let y = y as i64;
    let height = height as i64;
    let scaled = min + (range * y / (height - 1));
    scaled as i32
}

fn hsv_to_rgb(hue: u32, sat: u8, val: u8) -> Rgb888 {
    let c = (val as u32 * sat as u32) / 255;
    let h = hue % 360;

    let h_mod120 = h % 120;
    let delta = if h_mod120 < 60 {
        60 - h_mod120
    } else {
        h_mod120 - 60
    };

    let x = (c * (255 - (delta * 255 / 60))) / 255;

    let (r1, g1, b1) = match h {
        0..=59 => (c, x, 0),
        60..=119 => (x, c, 0),
        120..=179 => (0, c, x),
        180..=239 => (0, x, c),
        240..=299 => (x, 0, c),
        300..=359 => (c, 0, x),
        _ => (0, 0, 0),
    };

    let m = val as u32 - c;

    Rgb888::new(
        (r1 + m).min(255) as u8,
        (g1 + m).min(255) as u8,
        (b1 + m).min(255) as u8,
    )
}

fn mandelbrot_fixed(fb: &mut Framebuffer) {
    for py in 0..HEIGHT {
        for px in 0..WIDTH {
            let mut zx: i64 = 0;
            let mut zy: i64 = 0;
            let cx = map_to_real(px, WIDTH) as i64;
            let cy = map_to_imag(py, HEIGHT) as i64;

            let mut iter = 0;

            while iter < MAX_ITER {
                let zx2 = (zx * zx) >> 16;
                let zy2 = (zy * zy) >> 16;

                if zx2 + zy2 > (4 * SCALE as i64) {
                    break;
                }

                let two_zx_zy = (zx * zy) >> 15; // = 2*zx*zy
                let new_zx = zx2 - zy2 + cx;
                let new_zy = two_zx_zy + cy;

                zx = new_zx;
                zy = new_zy;

                iter += 1;
            }

            let color = if iter == MAX_ITER {
                Rgb888::BLACK
            } else {
                let hue = (iter * 9) % 360;
                hsv_to_rgb(hue, 200, 255)
            };

            Pixel(Point::new(px as i32, py as i32), color)
                .draw(fb)
                .unwrap();
        }
    }
}

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
    let _args = parse_args!();

    // create window
    let title = "mandelbrot\0";
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

    mandelbrot_fixed(&mut eg_fb);

    loop {
        print!(""); // yield
    }
}
