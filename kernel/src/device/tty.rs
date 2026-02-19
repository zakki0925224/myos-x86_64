use super::{uart, DeviceDriverFunction, DeviceDriverInfo};
use crate::{error::Result, fs::vfs, graphics::frame_buf_console, kinfo, sync::mutex::Mutex, task};
use alloc::{string::String, vec::Vec};
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

const IO_BUF_LEN: usize = 512;

static TTY: Mutex<Tty> = Mutex::new(Tty::new(true));
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

struct Tty {
    device_driver_info: DeviceDriverInfo,
    input_buf: Buffer<IO_BUF_LEN>,
    output_buf: Buffer<IO_BUF_LEN>,
    err_output_buf: Buffer<IO_BUF_LEN>,
    use_serial_port: bool,
    is_ready_get_line: bool,
    esc_state: EscState,
}

impl Tty {
    const fn new(use_serial_port: bool) -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("tty"),
            input_buf: Buffer::default(),
            output_buf: Buffer::default(),
            err_output_buf: Buffer::default(),
            use_serial_port,
            is_ready_get_line: false,
            esc_state: EscState::Normal,
        }
    }

    fn write(&mut self, c: char, buf_type: BufferType) -> Result<()> {
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

            let _ = frame_buf_console::write_char(c);
        }

        Ok(())
    }

    fn get_line(&mut self, buf_type: BufferType) -> String {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        let mut s = String::new();

        loop {
            if let Some(c) = buf.pop_front() {
                s.push(c);
            } else {
                break;
            }
        }

        s
    }

    fn get_char(&mut self, buf_type: BufferType) -> Option<char> {
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
                self.input_buf.pop_back();
                let _ = self.write('\x08', BufferType::Output);
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
            let _ = self.write(c, BufferType::Output);
        }

        Ok(())
    }
}

impl fmt::Write for Tty {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let buf_type = BufferType::Output;
        for c in s.chars() {
            self.write(c, buf_type).map_err(|_| fmt::Error)?;
        }

        Ok(())
    }
}

impl DeviceDriverFunction for Tty {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        let dev_desc = vfs::DeviceFileDescriptor {
            get_device_driver_info,
            open,
            close,
            read,
            write,
        };
        vfs::add_dev_file(dev_desc, self.device_driver_info.name)?;
        self.device_driver_info.attached = true;
        Ok(())
    }

    fn poll_normal(&mut self) -> Result<Self::PollNormalOutput> {
        unimplemented!()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        unimplemented!()
    }

    fn open(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn close(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    if let Ok(mut tty) = TTY.try_lock() {
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

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = TTY.try_lock()?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = TTY.try_lock()?;
    driver.probe()?;
    driver.attach(())?;
    kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = TTY.try_lock()?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = TTY.try_lock()?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = TTY.try_lock()?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = TTY.try_lock()?;
    for &c in data {
        driver.write(c as char, BufferType::Output)?;
    }

    Ok(())
}

pub fn input(c: char) -> Result<()> {
    if c == '\x03' {
        FLAG_SIGINT.store(true, Ordering::Relaxed);
        let mut tty = TTY.try_lock()?;
        tty.clear_input();
        return Ok(());
    }

    let c = if c == '\r' { '\n' } else { c };

    let mut tty = TTY.try_lock()?;
    tty.input_char(c)
}

pub fn check_sigint() {
    let sigint = FLAG_SIGINT.swap(false, Ordering::Relaxed);

    if sigint {
        task::single_scheduler::return_task(-1);
    }
}

pub fn get_line() -> Result<Option<String>> {
    let mut tty = TTY.try_lock()?;

    if tty.is_ready_get_line {
        tty.is_ready_get_line = false;
        Ok(Some(tty.get_line(BufferType::Input)))
    } else {
        Ok(None)
    }
}

pub fn get_char() -> Result<Option<char>> {
    let mut tty = TTY.try_lock()?;
    Ok(tty.get_char(BufferType::Input))
}

pub fn input_count() -> Result<usize> {
    let tty = TTY.try_lock()?;
    Ok(tty.input_count())
}
