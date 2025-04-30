use super::uart;
use crate::{
    error::{Error, Result},
    graphics::{color::*, frame_buf_console},
    theme::GLOBAL_THEME,
    util::{lifo::Lifo, mutex::Mutex},
};
use alloc::{boxed::Box, string::String};
use core::fmt::{self, Write};

const IO_BUF_LEN: usize = 512;
const IO_BUF_DEFAULT_VALUE: ConsoleCharacter = ConsoleCharacter {
    back_color: GLOBAL_THEME.io_buf_default_back_color,
    fore_color: GLOBAL_THEME.io_buf_default_fore_color,
    c: '\0',
};

type IoBufferType = Lifo<ConsoleCharacter, IO_BUF_LEN>;

// kernel console
static mut CONSOLE: Mutex<Console> = Mutex::new(Console::new(true));

#[derive(Debug, Clone, Copy)]
pub struct ConsoleCharacter {
    pub back_color: ColorCode,
    pub fore_color: ColorCode,
    pub c: char,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleError {
    IoBufferError {
        buf_type: BufferType,
        err: Box<Error>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferType {
    Input,
    Output,
    ErrorOutput,
}

// TTY + PTS
#[derive(Debug)]
pub struct Console {
    input_buf: IoBufferType,
    output_buf: IoBufferType,
    err_output_buf: IoBufferType,
    buf_default_value: ConsoleCharacter,
    use_serial_port: bool,
    is_ready_get_line: bool,
}

impl Console {
    pub const fn new(use_serial_port: bool) -> Self {
        Self {
            input_buf: Lifo::new(IO_BUF_DEFAULT_VALUE),
            output_buf: Lifo::new(IO_BUF_DEFAULT_VALUE),
            err_output_buf: Lifo::new(IO_BUF_DEFAULT_VALUE),
            buf_default_value: IO_BUF_DEFAULT_VALUE,
            use_serial_port,
            is_ready_get_line: false,
        }
    }

    pub fn is_full(&self, buf_type: BufferType) -> bool {
        let buf = match buf_type {
            BufferType::Input => &self.input_buf,
            BufferType::Output => &self.output_buf,
            BufferType::ErrorOutput => &self.err_output_buf,
        };

        buf.is_full()
    }

    pub fn reset_buf(&mut self, buf_type: BufferType) {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        buf.reset();
    }

    pub fn set_back_color(&mut self, back_color: ColorCode) {
        self.buf_default_value.back_color = back_color;
    }

    pub fn set_fore_color(&mut self, fore_color: ColorCode) {
        self.buf_default_value.fore_color = fore_color;
    }

    pub fn reset_color(&mut self) {
        self.buf_default_value.back_color = IO_BUF_DEFAULT_VALUE.back_color;
        self.buf_default_value.fore_color = IO_BUF_DEFAULT_VALUE.fore_color;
    }

    pub fn write(&mut self, c: char, buf_type: BufferType) -> Result<()> {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };
        let mut value = self.buf_default_value;
        value.c = c;

        match c {
            '\x08' /* backspace */ | '\x7f' /* delete */ => {
                buf.pop()?;
            }
            _ => {
                buf.push(value).map_err(|err| ConsoleError::IoBufferError {
                    buf_type,
                    err: Box::new(err),
                })?;
            }
        }

        if (buf_type == BufferType::Output || buf_type == BufferType::ErrorOutput)
            && self.use_serial_port
        {
            let data = match c {
                '\x08' | '\x7f' => '\x08' as u8,
                _ => c as u8,
            };

            // backspace
            if data == 0x08 {
                uart::send_data(data);
                uart::send_data(b' ');
                uart::send_data(data);
            } else {
                uart::send_data(data);
            }
        }

        Ok(())
    }

    pub fn get_line(&mut self, buf_type: BufferType) -> String {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        let mut s = String::new();

        loop {
            let c = match buf.pop() {
                Ok(value) => value.c,
                Err(_) => break,
            };
            s.push(c);
        }

        s.chars().rev().collect()
    }

    pub fn get_char(&mut self, buf_type: BufferType) -> char {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        let ascii_code = match buf.pop() {
            Ok(value) => value.c,
            Err(_) => '\0', // null
        };

        ascii_code
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let buf_type = BufferType::Output;
        for c in s.chars() {
            if self.is_full(buf_type) {
                self.reset_buf(buf_type);
            }

            let _ = self.write(c, buf_type);
        }

        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    if let Ok(mut console) = unsafe { CONSOLE.try_lock() } {
        let _ = console.write_fmt(args);
    }

    let _ = frame_buf_console::write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::device::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println
{
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn clear_input_buf() -> Result<()> {
    unsafe { CONSOLE.try_lock() }?.reset_buf(BufferType::Input);
    Ok(())
}

pub fn input(c: char) -> Result<()> {
    let mut c = c;
    if c == '\r' {
        c = '\n';
    }

    match c {
        '\n' => {
            println!();
        }
        _ => {
            print!("{}", c);
        }
    }

    let mut console = unsafe { CONSOLE.try_lock() }?;

    if console.is_full(BufferType::Input) {
        console.reset_buf(BufferType::Input);
    }

    console.write(c, BufferType::Input)?;

    if c == '\n' {
        console.is_ready_get_line = true;
    }

    Ok(())
}

pub fn get_line() -> Result<Option<String>> {
    let mut console = unsafe { CONSOLE.try_lock() }?;

    if console.is_ready_get_line {
        console.is_ready_get_line = false;
        Ok(Some(console.get_line(BufferType::Input)))
    } else {
        Ok(None)
    }
}

pub fn get_char() -> Result<char> {
    let mut console = unsafe { CONSOLE.try_lock() }?;
    Ok(console.get_char(BufferType::Input))
}
