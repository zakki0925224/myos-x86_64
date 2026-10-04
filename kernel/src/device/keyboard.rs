use crate::{
    device::{tty, DeviceInfo, Driver},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    task::{scheduler, WaitKey},
    util::{
        keyboard::{key_event::*, scan_code::KeyCode},
        slice::Sliceable,
    },
};
use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use libc_rs::key_event;

const NAME: &str = "keyboard";
const QUEUE_CAPACITY: usize = 256;

static KEYBOARD_DRIVER: Mutex<KeyboardDriver> = Mutex::new(KeyboardDriver::new());

unsafe impl Sliceable for key_event {}

struct KeyboardDriver {
    queue: VecDeque<KeyEvent>,
    read_queue: VecDeque<key_event>,
    is_opened: bool,
}

impl KeyboardDriver {
    const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            read_queue: VecDeque::new(),
            is_opened: false,
        }
    }
}

impl Driver for KeyboardDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<()> {
        loop {
            let event = match self.queue.pop_front() {
                Some(e) => e,
                None => return Ok(()),
            };

            if event.state != KeyState::Pressed {
                continue;
            }

            match event.code {
                KeyCode::CursorUp => tty::input_str("\x1b[A")?,
                KeyCode::CursorDown => tty::input_str("\x1b[B")?,
                KeyCode::CursorRight => tty::input_str("\x1b[C")?,
                KeyCode::CursorLeft => tty::input_str("\x1b[D")?,
                _ => {
                    if let Some(c) = event.c {
                        tty::input(c)?;
                    }
                }
            }
        }
    }

    fn open(&mut self) -> Result<()> {
        self.read_queue.clear();
        self.is_opened = true;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.is_opened = false;
        self.read_queue.clear();
        Ok(())
    }

    fn read(&mut self, _offset: usize, max_len: usize) -> Result<Vec<u8>> {
        if self.read_queue.is_empty() {
            return Err(Error::BufferEmpty.into());
        }

        let count = (max_len / size_of::<key_event>()).min(self.read_queue.len());
        let mut bytes = Vec::with_capacity(count * size_of::<key_event>());

        for event in self.read_queue.drain(..count) {
            bytes.extend_from_slice(event.as_slice());
        }

        Ok(bytes)
    }

    fn readable(&mut self) -> bool {
        !self.read_queue.is_empty()
    }
}

pub fn probe_and_attach() -> Result<()> {
    KEYBOARD_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&KEYBOARD_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn push_key_event(event: KeyEvent) -> Result<()> {
    let pushed = {
        let mut driver = KEYBOARD_DRIVER.try_lock()?;
        let opened = driver.is_opened;

        if opened {
            if driver.read_queue.len() == QUEUE_CAPACITY {
                driver.read_queue.pop_front();
            }

            driver.read_queue.push_back(key_event {
                code: event.code as u16,
                pressed: (event.state == KeyState::Pressed) as u8,
                _reserved: 0,
                c: event.c.map_or(0, |c| c as u32),
            });
        }

        driver.queue.push_back(event);
        opened
    };

    if pushed {
        scheduler::wake(WaitKey::Device(NAME));
    }

    Ok(())
}

pub fn poll_normal() -> Result<()> {
    KEYBOARD_DRIVER.try_lock()?.poll()
}
