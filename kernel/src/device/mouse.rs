use crate::{
    device::{DeviceInfo, Driver},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    task::{scheduler, WaitKey},
    util::{mouse::mouse_event::MouseEvent, slice::Sliceable},
};
use alloc::{collections::VecDeque, vec::Vec};
use libc_rs::*;

const NAME: &str = "mouse";
const QUEUE_CAPACITY: usize = 256;

static MOUSE_DRIVER: Mutex<MouseDriver> = Mutex::new(MouseDriver::new());

unsafe impl Sliceable for mouse_event {}

struct MouseDriver {
    queue: VecDeque<mouse_event>,
    is_opened: bool,
}

impl MouseDriver {
    const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            is_opened: false,
        }
    }

    fn push(&mut self, event: mouse_event) -> bool {
        if !self.is_opened {
            return false;
        }

        if self.queue.len() == QUEUE_CAPACITY {
            self.queue.pop_front();
        }

        self.queue.push_back(event);
        true
    }
}

impl Driver for MouseDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn open(&mut self) -> Result<()> {
        self.queue.clear();
        self.is_opened = true;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.is_opened = false;
        self.queue.clear();
        Ok(())
    }

    fn read(&mut self, _offset: usize, max_len: usize) -> Result<Vec<u8>> {
        if self.queue.is_empty() {
            return Err(Error::BufferEmpty.into());
        }

        let count = (max_len / size_of::<mouse_event>()).min(self.queue.len());
        let mut bytes = Vec::with_capacity(count * size_of::<mouse_event>());

        for event in self.queue.drain(..count) {
            bytes.extend_from_slice(event.as_slice());
        }

        Ok(bytes)
    }

    fn readable(&mut self) -> bool {
        !self.queue.is_empty()
    }
}

fn to_user_event(event: &MouseEvent) -> mouse_event {
    let (left, right, middle, is_abs, x, y) = match event {
        MouseEvent::Ps2MouseDevice(e) => (
            e.left,
            e.right,
            e.middle,
            false,
            e.rel_x as i32,
            e.rel_y as i32,
        ),
        MouseEvent::UsbHidMouse(e) => (
            e.left,
            e.right,
            e.middle,
            true,
            e.abs_x as i32,
            e.abs_y as i32,
        ),
    };

    let mut buttons = 0;
    if left {
        buttons |= MOUSE_BUTTON_LEFT as u8;
    }
    if right {
        buttons |= MOUSE_BUTTON_RIGHT as u8;
    }
    if middle {
        buttons |= MOUSE_BUTTON_MIDDLE as u8;
    }

    mouse_event {
        buttons,
        is_abs: is_abs as u8,
        _reserved: [0; 2],
        x,
        y,
    }
}

pub fn probe_and_attach() -> Result<()> {
    MOUSE_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&MOUSE_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn push_event(event: MouseEvent) -> Result<()> {
    let pushed = MOUSE_DRIVER.try_lock()?.push(to_user_event(&event));
    if pushed {
        scheduler::wake(WaitKey::Device(NAME));
    }

    Ok(())
}
