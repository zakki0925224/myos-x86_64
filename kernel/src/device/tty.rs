use super::{uart, DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    error::{Error, Result},
    fs::vfs,
    graphics::frame_buf_console,
    info,
    sync::mutex::Mutex,
    util::lifo::Lifo,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::fmt::{self, Write};

const IO_BUF_LEN: usize = 512;

static mut TTY: Mutex<Tty> = Mutex::new(Tty::new(true));

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferType {
    Input,
    Output,
    ErrorOutput,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TtyError {
    IoBufferError {
        buf_type: BufferType,
        err: Box<Error>,
    },
}

struct Tty {
    device_driver_info: DeviceDriverInfo,
    input_buf: Lifo<char, IO_BUF_LEN>,
    output_buf: Lifo<char, IO_BUF_LEN>,
    err_output_buf: Lifo<char, IO_BUF_LEN>,
    use_serial_port: bool,
    is_ready_get_line: bool,
}

impl Tty {
    const fn new(use_serial_port: bool) -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("tty"),
            input_buf: Lifo::new('\0'),
            output_buf: Lifo::new('\0'),
            err_output_buf: Lifo::new('\0'),
            use_serial_port,
            is_ready_get_line: false,
        }
    }

    fn is_full(&self, buf_type: BufferType) -> bool {
        match buf_type {
            BufferType::Input => self.input_buf.is_full(),
            BufferType::Output => self.output_buf.is_full(),
            BufferType::ErrorOutput => self.err_output_buf.is_full(),
        }
    }

    fn reset_buf(&mut self, buf_type: BufferType) {
        match buf_type {
            BufferType::Input => self.input_buf.reset(),
            BufferType::Output => self.output_buf.reset(),
            BufferType::ErrorOutput => self.err_output_buf.reset(),
        }
    }

    fn write(&mut self, c: char, buf_type: BufferType) -> Result<()> {
        let _ = frame_buf_console::write_char(c);

        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        match c {
            '\x08' /* backspace */ | '\x7f' /* delete */ => {
                buf.pop()?;
            }
            _ => {
                buf.push(c).map_err(|err| TtyError::IoBufferError { buf_type, err: Box::new(err) })?;
            }
        }

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
            if let Ok(c) = buf.pop() {
                s.push(c);
            } else {
                break;
            }
        }

        s.chars().rev().collect()
    }

    fn get_char(&mut self, buf_type: BufferType) -> char {
        let buf = match buf_type {
            BufferType::Input => &mut self.input_buf,
            BufferType::Output => &mut self.output_buf,
            BufferType::ErrorOutput => &mut self.err_output_buf,
        };

        match buf.pop() {
            Ok(c) => c,
            Err(_) => '\0', // null
        }
    }
}

impl fmt::Write for Tty {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let buf_type = BufferType::Output;
        for c in s.chars() {
            if self.is_full(buf_type) {
                self.reset_buf(buf_type);
            }

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
    if let Ok(mut tty) = unsafe { TTY.try_lock() } {
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
    let driver = unsafe { TTY.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { TTY.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { TTY.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { TTY.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { TTY.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { TTY.try_lock() }?;
    for &c in data {
        driver.write(c as char, BufferType::Output)?;
    }

    Ok(())
}

pub fn input(c: char) -> Result<()> {
    let mut c = c;
    if c == '\r' {
        c = '\n';
    }

    let mut tty = unsafe { TTY.try_lock() }?;
    if tty.is_full(BufferType::Input) {
        tty.reset_buf(BufferType::Input);
    }

    tty.write(c, BufferType::Input)?;

    if c == '\n' {
        tty.is_ready_get_line = true;
    }

    Ok(())
}

pub fn get_line() -> Result<Option<String>> {
    let mut tty = unsafe { TTY.try_lock() }?;

    if tty.is_ready_get_line {
        tty.is_ready_get_line = false;
        Ok(Some(tty.get_line(BufferType::Input)))
    } else {
        Ok(None)
    }
}

pub fn get_char() -> Result<char> {
    let mut tty = unsafe { TTY.try_lock() }?;
    Ok(tty.get_char(BufferType::Input))
}
