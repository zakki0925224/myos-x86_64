use crate::{device::uart, graphics::fb_console};
use core::fmt::{self, Write};

struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        uart::send_data(b);
    }
    let _ = fb_console::write_str(s);
}

pub fn write_bytes(bytes: &[u8]) {
    for chunk in bytes.utf8_chunks() {
        write_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            write_str("\u{fffd}");
        }
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
