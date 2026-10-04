use super::{uart, DeviceInfo, Driver};
use crate::{error::Result, fs::vfs, graphics::frame_buf_console, kinfo, sync::mutex::Mutex, task};
use alloc::string::String;
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

const IO_BUF_LEN: usize = 512;

const NAME: &str = "tty";

static TTY_DRIVER: Mutex<TtyDriver> = Mutex::new(TtyDriver::new(true));
static FLAG_SIGINT: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferType {
    Input,
    Output,
    ErrorOutput,
}

struct Buffer<const N: usize> {
    buf: [char; N],
    head: usize,
    tail: usize,
    full: bool,
}

impl<const N: usize> Buffer<N> {
    const fn default() -> Self {
        Self {
            buf: ['\0'; N],
            head: 0,
            tail: 0,
            full: false,
        }
    }

    fn push(&mut self, c: char) {
        if self.full {
            self.head = (self.head + 1) % N;
        }
        self.buf[self.tail] = c;
        self.tail = (self.tail + 1) % N;
        self.full = self.tail == self.head;
    }

    fn pop_front(&mut self) -> Option<char> {
        if !self.full && (self.head == self.tail) {
            return None;
        }
        let c = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.full = false;
        Some(c)
    }

    fn pop_back(&mut self) -> Option<char> {
        if !self.full && (self.head == self.tail) {
            return None;
        }
        self.tail = (self.tail + N - 1) % N;
        let c = self.buf[self.tail];
        self.full = false;
        Some(c)
    }

    fn len(&self) -> usize {
        if self.full {
            N
        } else if self.tail >= self.head {
            self.tail - self.head
        } else {
            N + self.tail - self.head
        }
    }

    fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.full = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EscState {
    Normal,
    Esc,
    EscBracket,
}

struct TtyDriver {
    input_buf: Buffer<IO_BUF_LEN>,
    output_buf: Buffer<IO_BUF_LEN>,
    err_output_buf: Buffer<IO_BUF_LEN>,
    use_serial_port: bool,
    is_ready_get_line: bool,
    esc_state: EscState,
}

impl TtyDriver {
    const fn new(use_serial_port: bool) -> Self {
        Self {
            input_buf: Buffer::default(),
            output_buf: Buffer::default(),
            err_output_buf: Buffer::default(),
            use_serial_port,
            is_ready_get_line: false,
            esc_state: EscState::Normal,
        }
    }

    fn write_char(&mut self, c: char, buf_type: BufferType) -> Result<()> {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        match c {
            '\x08' /* backspace */ | '\x7f' /* delete */ => {
                let _ = buf.pop_back();
            }
            _ => {
                buf.push(c);
            }
        }

        if buf_type != BufferType::Input {
            if self.use_serial_port {
                let data = match c {
                    '\x08' | '\x7f' => 0x08,
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

            let _ = frame_buf_console::write_char(c);
        }

        Ok(())
    }

    fn line(&mut self, buf_type: BufferType) -> String {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        let mut s = String::new();

        while let Some(c) = buf.pop_front() {
            match c {
                '\x08' | '\x7f' => {
                    s.pop();
                }
                _ => {
                    s.push(c);
                }
            }
        }

        s
    }

    fn char(&mut self, buf_type: BufferType) -> Option<char> {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        let c = buf.pop_front();
        if buf_type == BufferType::Input && c == Some('\n') {
            self.is_ready_get_line = false;
        }
        c
    }

    pub fn input_count(&self) -> usize {
        self.input_buf.len()
    }

    fn clear_input(&mut self) {
        self.input_buf.clear();
        self.is_ready_get_line = false;
    }

    fn input_char(&mut self, c: char) -> Result<()> {
        match c {
            '\x08' | '\x7f' => {
                self.input_buf.push(c);
                let _ = self.write_char('\x08', BufferType::Output);
                return Ok(());
            }
            _ => {}
        }

        self.input_buf.push(c);
        if c == '\n' {
            self.is_ready_get_line = true;
        }

        let echo = match self.esc_state {
            EscState::Normal => {
                if c == '\x1b' {
                    self.esc_state = EscState::Esc;
                    false
                } else {
                    true
                }
            }
            EscState::Esc => {
                self.esc_state = if c == '[' {
                    EscState::EscBracket
                } else {
                    EscState::Normal
                };
                false
            }
            EscState::EscBracket => {
                self.esc_state = EscState::Normal;
                false
            }
        };

        if echo {
            let _ = self.write_char(c, BufferType::Output);
        }

        Ok(())
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        for b in data {
            self.write_char(*b as char, BufferType::Output)?;
        }

        Ok(())
    }
}

impl fmt::Write for TtyDriver {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let buf_type = BufferType::Output;
        for c in s.chars() {
            self.write_char(c, buf_type).map_err(|_| fmt::Error)?;
        }

        Ok(())
    }
}

impl Driver for TtyDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.write_bytes(data)
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    if let Ok(mut tty) = TTY_DRIVER.try_lock() {
        let _ = tty.write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::device::tty::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn probe_and_attach() -> Result<()> {
    TTY_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&TTY_DRIVER)?;
    kinfo!("{}: Attached!", NAME);
    Ok(())
}

pub fn input_str(s: &str) -> Result<()> {
    for c in s.chars() {
        input(c)?;
    }

    Ok(())
}

pub fn input(c: char) -> Result<()> {
    if c == '\x03' {
        FLAG_SIGINT.store(true, Ordering::Relaxed);
        let mut tty = TTY_DRIVER.try_lock()?;
        tty.clear_input();
        return Ok(());
    }

    let c = if c == '\r' { '\n' } else { c };

    let mut tty = TTY_DRIVER.try_lock()?;
    tty.input_char(c)
}

pub fn check_sigint() {
    let sigint = FLAG_SIGINT.swap(false, Ordering::Relaxed);

    if sigint {
        task::scheduler::exit_current(-1);
    }
}

pub fn line() -> Result<Option<String>> {
    let mut tty = TTY_DRIVER.try_lock()?;

    if tty.is_ready_get_line {
        tty.is_ready_get_line = false;
        Ok(Some(tty.line(BufferType::Input)))
    } else {
        Ok(None)
    }
}

pub fn char() -> Result<Option<char>> {
    let mut tty = TTY_DRIVER.try_lock()?;
    Ok(tty.char(BufferType::Input))
}

pub fn input_count() -> Result<usize> {
    let tty = TTY_DRIVER.try_lock()?;
    Ok(tty.input_count())
}
