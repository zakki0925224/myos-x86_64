#![no_std]
#![no_main]

#[macro_use]
extern crate alloc;

mod bmp;

use alloc::string::String;
use bmp::Bitmap;
use libc_rs::*;

const DEFAULT_CURSOR_PATH: &str = "/mnt/initramfs/sys/mouse_pointer.bmp";

const EVENT_ZERO: mouse_event = mouse_event {
    buttons: 0,
    is_abs: 0,
    _reserved: [0; 2],
    x: 0,
    y: 0,
};

fn load_cursor(path: &str) -> core::result::Result<Bitmap, String> {
    let file = File::open(path).map_err(|_| String::from("failed to open cursor image"))?;
    let mut data = vec![0; file.size()];
    file.read(&mut data)
        .map_err(|_| String::from("failed to read cursor image"))?;
    Bitmap::parse(&data).map_err(|e| format!("failed to parse cursor image: {}", e))
}

struct Cursor {
    layer: Layer,
    x: isize,
    y: isize,
    max_x: isize,
    max_y: isize,
}

impl Cursor {
    fn new(image: &Bitmap, screen_w: usize, screen_h: usize) -> Result<Self> {
        let x = screen_w / 2;
        let y = screen_h / 2;
        let layer = Layer::create(x, y, image.width, image.height, &image.pixels, true)?;

        Ok(Self {
            layer,
            x: x as isize,
            y: y as isize,
            max_x: screen_w as isize - 1,
            max_y: screen_h as isize - 1,
        })
    }

    fn apply(&mut self, e: &mouse_event) {
        if e.is_abs != 0 {
            self.x = e.x as isize;
            self.y = e.y as isize;
        } else {
            self.x += e.x as isize;
            self.y += e.y as isize;
        }

        self.x = self.x.clamp(0, self.max_x);
        self.y = self.y.clamp(0, self.max_y);
    }

    fn commit(&self) {
        let _ = self.layer.move_to(self.x as usize, self.y as usize);
    }
}

fn fail(msg: &str, err: LibcError) -> ! {
    println!("wm: {}: {:?}", msg, err);
    unsafe { exit(-1) }
}

#[unsafe(no_mangle)]
pub fn _start() {
    let args = parse_args!();
    let cursor_path = args.get(1).copied().unwrap_or(DEFAULT_CURSOR_PATH);

    let (screen_w, screen_h) =
        screen_size().unwrap_or_else(|e| fail("failed to get screen size", e));
    let mouse = Fd::open("/dev/mouse", OPEN_FLAG_NONE)
        .unwrap_or_else(|e| fail("failed to open /dev/mouse", e));
    let cursor_image = load_cursor(cursor_path).unwrap_or_else(|msg| {
        println!("wm: {}: {}", msg, cursor_path);
        unsafe { exit(-1) }
    });
    let mut cursor = Cursor::new(&cursor_image, screen_w, screen_h)
        .unwrap_or_else(|e| fail("failed to create cursor", e));

    let mut events = [EVENT_ZERO; 16];

    loop {
        let mut fds = [mouse.poll_in()];
        if let Err(e) = poll(&mut fds, -1) {
            fail("poll failed", e);
        }

        let n = match mouse.read_events(&mut events) {
            Ok(n) => n,
            Err(_) => continue,
        };

        for e in &events[..n] {
            cursor.apply(e);
        }
        cursor.commit();
    }
}
