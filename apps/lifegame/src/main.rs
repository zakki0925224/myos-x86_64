#![no_std]
#![no_main]

use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*, primitives::*};
use libc_rs::*;

const WIDTH: usize = 450;
const HEIGHT: usize = 400;

const COLS: usize = WIDTH / 5;
const ROWS: usize = HEIGHT / 5;
const CELL_SIZE: i32 = 5;

static mut CURRENT_BOARD: [[u8; COLS]; ROWS] = [[0; COLS]; ROWS];
static mut NEXT_BOARD: [[u8; COLS]; ROWS] = [[0; COLS]; ROWS];
static mut GENERATION: u64 = 0;

const DELAY_MS: u64 = 100;

fn initialize_board() {
    unsafe {
        // acorn pattern - evolves for 5206 generations
        // pattern:
        //  .O.....
        //  ...O...
        //  OO..OOO

        let cx = ROWS / 2;
        let cy = COLS / 2;

        CURRENT_BOARD[cx][cy + 1] = 1;
        CURRENT_BOARD[cx + 1][cy + 3] = 1;
        CURRENT_BOARD[cx + 2][cy] = 1;
        CURRENT_BOARD[cx + 2][cy + 1] = 1;
        CURRENT_BOARD[cx + 2][cy + 4] = 1;
        CURRENT_BOARD[cx + 2][cy + 5] = 1;
        CURRENT_BOARD[cx + 2][cy + 6] = 1;
    }
}

fn count_neighbors(r: usize, c: usize) -> u8 {
    let mut count = 0;
    unsafe {
        for dr in [ROWS - 1, 0, 1] {
            for dc in [COLS - 1, 0, 1] {
                if dr == 0 && dc == 0 {
                    continue;
                }

                let nr = (r + dr) % ROWS;
                let nc = (c + dc) % COLS;

                count += CURRENT_BOARD[nr][nc];
            }
        }
    }
    count
}

fn compute_next_generation() {
    unsafe {
        for r in 0..ROWS {
            for c in 0..COLS {
                let neighbors = count_neighbors(r, c);
                let current_state = CURRENT_BOARD[r][c];

                let next_state: u8;

                if current_state == 1 {
                    if neighbors == 2 || neighbors == 3 {
                        next_state = 1;
                    } else {
                        next_state = 0;
                    }
                } else {
                    if neighbors == 3 {
                        next_state = 1;
                    } else {
                        next_state = 0;
                    }
                }

                NEXT_BOARD[r][c] = next_state;
            }
        }

        for r in 0..ROWS {
            for c in 0..COLS {
                CURRENT_BOARD[r][c] = NEXT_BOARD[r][c];
            }
        }
    }
}

fn draw_board(fb: &mut Framebuffer, generation: u64) {
    let alive_color = Rgb888::new(0, 255, 100);
    let dead_color = Rgb888::new(20, 20, 20);

    Rectangle::new(Point::zero(), Size::new(WIDTH as u32, HEIGHT as u32))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(dead_color).build())
        .draw(fb)
        .unwrap();

    unsafe {
        for r in 0..ROWS {
            for c in 0..COLS {
                if CURRENT_BOARD[r][c] == 1 {
                    Rectangle::new(
                        Point::new(
                            (c * CELL_SIZE as usize) as i32,
                            (r * CELL_SIZE as usize) as i32,
                        ),
                        Size::new(CELL_SIZE as u32, CELL_SIZE as u32),
                    )
                    .into_styled(PrimitiveStyleBuilder::new().fill_color(alive_color).build())
                    .draw(fb)
                    .unwrap();
                }
            }
        }
    }

    if generation % 100 == 0 {
        println!("Generation: {}", generation);
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
                    *pixel_ptr.add(3) = 0xff;
                }
            }
        }

        Ok(())
    }
}

#[no_mangle]
pub unsafe fn _start() {
    let _args = parse_args!();

    let title = "lifegame\0";
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

    let fb = malloc((WIDTH * HEIGHT * 4) as u64);
    if fb.is_null() {
        println!("Failed to allocate framebuffer memory");
        exit(-1);
    }

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

    initialize_board();
    draw_board(&mut eg_fb, 0);

    loop {
        let start_time = sys_uptime();
        while sys_uptime() - start_time < DELAY_MS {
            // wait
        }

        unsafe {
            GENERATION += 1;
        }
        compute_next_generation();
        unsafe {
            draw_board(&mut eg_fb, GENERATION);
        }
    }
}
