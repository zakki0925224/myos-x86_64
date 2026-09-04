use super::{register_pollable, Device, DeviceInfo, Pollable};
use crate::{
    device::tty,
    error::Result,
    sync::mutex::Mutex,
    util::keyboard::{key_event::*, scan_code::KeyCode},
};
use alloc::{collections::vec_deque::VecDeque, sync::Arc};

const NAME: &str = "keyboard";

static KEYBOARD: Mutex<Option<Arc<KeyboardDevice>>> = Mutex::new(None);

pub struct KeyboardDevice {
    queue: Mutex<VecDeque<KeyEvent>>,
}

impl KeyboardDevice {
    const fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }
}

impl Device for KeyboardDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl Pollable for KeyboardDevice {
    fn poll(&self) -> Result<()> {
        loop {
            let event = match self.queue.try_lock()?.pop_front() {
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
}

pub fn probe_and_attach() -> Result<()> {
    let dev = Arc::new(KeyboardDevice::new());
    register_pollable(dev.clone())?;
    *KEYBOARD.try_lock()? = Some(dev);

    Ok(())
}

pub fn push_key_event(event: KeyEvent) -> Result<()> {
    let dev = KEYBOARD.try_lock()?.clone();

    if let Some(dev) = dev {
        dev.queue.try_lock()?.push_back(event);
    }

    Ok(())
}
