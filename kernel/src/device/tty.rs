use crate::{
    console,
    device::{DeviceInfo, Driver},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    task::{scheduler, WaitKey},
    util::ansi::EscState,
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

const IO_BUF_LEN: usize = 512;
pub const NAME: &str = "tty";

static TTY_DRIVER: Mutex<TtyDriver> = Mutex::new(TtyDriver::new());
static FLAG_SIGINT: AtomicBool = AtomicBool::new(false);

struct RingBuf<const N: usize> {
    buf: [u8; N],
    head: usize,
    len: usize,
}

impl<const N: usize> RingBuf<N> {
    const fn default() -> Self {
        Self {
            buf: [0; N],
            head: 0,
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    fn push(&mut self, b: u8) -> bool {
        if self.len == N {
            return false;
        }
        self.buf[(self.head + self.len) % N] = b;
        self.len += 1;
        true
    }

    fn pop_front(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let b = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(b)
    }

    fn pop_back(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.buf[(self.head + self.len) % N])
    }

    fn last(&self) -> Option<u8> {
        (self.len > 0).then(|| self.buf[(self.head + self.len - 1) % N])
    }

    fn free(&self) -> usize {
        N - self.len
    }
}

struct TtyDriver {
    buf: RingBuf<IO_BUF_LEN>,
    lines: usize,
    esc_state: EscState,
}

impl TtyDriver {
    const fn new() -> Self {
        Self {
            buf: RingBuf::default(),
            lines: 0,
            esc_state: EscState::Normal,
        }
    }

    fn input(&mut self, b: u8) -> bool {
        // ESC
        if self.esc_state != EscState::Normal && b >= 0x20 {
            self.esc_state = match (self.esc_state, b) {
                (EscState::Esc, b'[') => EscState::Csi,
                (EscState::Csi, 0x20..=0x3f) => EscState::Csi,
                _ => EscState::Normal,
            };
            return false;
        }
        self.esc_state = EscState::Normal;

        match b {
            0x03 /* ^C */ => {
                self.buf.clear();
                self.lines = 0;
                FLAG_SIGINT.store(true, Ordering::Relaxed);
                self.echo(b"^C\n");
                true
            }
            0x08 /* backspace */ | 0x7f /* delete */ => {
                self.erase();
                false
            }
            b'\r' | b'\n' => {
                if !self.buf.push(b'\n') {
                    return false;
                }

                self.lines += 1;
                self.echo(b"\n");
                true
            }
            0x1b /* ESC */ => {
                self.esc_state = EscState::Esc;
                false
            }
            _ if self.buf.free() > 1 => {
                self.buf.push(b);
                if b >= 0x20 || b == b'\t' {
                    self.echo(&[b]);
                }
                false
            }
            _ => false
        }
    }

    fn erase(&mut self) {
        let mut erased = false;

        while let Some(b) = self.buf.last() {
            if b == b'\n' {
                break;
            }

            self.buf.pop_back();
            erased = true;

            if b & 0xc0 != 0x80 {
                break;
            }
        }

        if erased {
            self.echo(b"\x08 \x08");
        }
    }

    fn read_line(&mut self, max_len: usize) -> Result<Vec<u8>> {
        if self.lines == 0 {
            return Err(Error::BufferEmpty.into());
        }

        let mut data = Vec::new();
        while data.len() < max_len {
            let Some(b) = self.buf.pop_front() else {
                break;
            };
            data.push(b);

            if b == b'\n' {
                self.lines -= 1;
                break;
            }
        }

        Ok(data)
    }

    fn echo(&self, bytes: &[u8]) {
        console::write_bytes(bytes);
    }
}

impl Driver for TtyDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self, _offset: usize, max_len: usize) -> Result<Vec<u8>> {
        self.read_line(max_len)
    }

    fn readable(&mut self) -> bool {
        self.lines > 0
    }
}

pub fn probe_and_attach() -> Result<()> {
    TTY_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&TTY_DRIVER)?;
    kinfo!("{}: Attached!", NAME);
    Ok(())
}

pub fn input(b: u8) -> Result<()> {
    let wake = TTY_DRIVER.try_lock()?.input(b);
    if wake {
        scheduler::wake(WaitKey::Device(NAME));
    }

    Ok(())
}

pub fn input_bytes(bytes: &[u8]) -> Result<()> {
    for &b in bytes {
        input(b)?;
    }

    Ok(())
}

pub fn read(max_len: usize) -> Result<Vec<u8>> {
    TTY_DRIVER.try_lock()?.read_line(max_len)
}

pub fn check_sigint() {
    if FLAG_SIGINT.swap(false, Ordering::Relaxed) {
        scheduler::exit_current(-1);
    }
}
