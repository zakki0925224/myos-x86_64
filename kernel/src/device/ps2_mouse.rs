use crate::{
    arch::IoPortAddress,
    device::{
        register_irq, register_pollable, CharDevice, Device, DeviceInfo, InterruptSource, Pollable,
    },
    error::{Error, Result},
    fs::vfs,
    graphics::window_manager::{self, MouseEvent},
    kinfo,
    sync::mutex::Mutex,
    task::async_task::Priority,
    util::fifo::Fifo,
};
use alloc::{sync::Arc, vec::Vec};

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);

const VEC_PS2_MOUSE: u8 = 0x2c;
const NAME: &str = "ps2-mouse";

#[derive(Default, Debug)]
pub struct Ps2MouseEvent {
    pub middle: bool,
    pub right: bool,
    pub left: bool,
    pub rel_x: i16,
    pub rel_y: i16,
}

enum MousePhase {
    WaitingAck,
    WaitingData0,
    WaitingData1,
    WaitingData2,
}

impl MousePhase {
    const fn default() -> Self {
        Self::WaitingAck
    }

    fn next(&mut self) {
        *self = match self {
            Self::WaitingAck => Self::WaitingData0,
            Self::WaitingData0 => Self::WaitingData1,
            Self::WaitingData1 => Self::WaitingData2,
            Self::WaitingData2 => Self::WaitingData0,
        }
    }
}

struct Inner {
    mouse_phase: MousePhase,
    data_buf: Fifo<u8, 256>,
    data_buf2: [u8; 3],
}

impl Inner {
    const fn new() -> Self {
        Self {
            mouse_phase: MousePhase::default(),
            data_buf: Fifo::new(0),
            data_buf2: [0; 3],
        }
    }

    fn receive(&mut self, data: u8) -> Result<()> {
        if self.data_buf.enqueue(data).is_err() {
            self.data_buf.reset_ptr();
            self.data_buf.enqueue(data)?;
        }

        Ok(())
    }

    fn event(&mut self) -> Result<Option<Ps2MouseEvent>> {
        let data = self.data_buf.dequeue()?;
        let e = match self.mouse_phase {
            MousePhase::WaitingAck => {
                if data == 0xfa {
                    self.mouse_phase.next();
                }

                None
            }
            MousePhase::WaitingData0 => {
                // validation check
                let one = data & 0x08 != 0;
                let x_of = data & 0x40 != 0;
                let y_of = data & 0x80 != 0;

                if one && !x_of && !y_of {
                    self.data_buf2[0] = data;
                    self.mouse_phase.next();
                }

                None
            }
            MousePhase::WaitingData1 => {
                self.data_buf2[1] = data;
                self.mouse_phase.next();
                None
            }
            MousePhase::WaitingData2 => {
                self.data_buf2[2] = data;
                self.mouse_phase.next();

                let button_m = self.data_buf2[0] & 0x4 != 0;
                let button_r = self.data_buf2[0] & 0x2 != 0;
                let button_l = self.data_buf2[0] & 0x1 != 0;
                let x_sign = self.data_buf2[0] & 0x10 != 0;
                let y_sign = self.data_buf2[0] & 0x20 != 0;

                let mut rel_x = self.data_buf2[1] as i16;
                let mut rel_y = self.data_buf2[2] as i16;

                if x_sign {
                    rel_x |= 0xff00u16 as i16;
                }

                if y_sign {
                    rel_y |= 0xff00u16 as i16;
                }

                rel_y = -rel_y;

                Some(Ps2MouseEvent {
                    middle: button_m,
                    right: button_r,
                    left: button_l,
                    rel_x,
                    rel_y,
                })
            }
        };

        Ok(e)
    }

    fn wait_ready(&self) {
        while PS2_CMD_AND_STATE_REG_ADDR.in8() & 0x2 != 0 {
            continue;
        }
    }
}

pub struct Ps2MouseDevice {
    inner: Mutex<Inner>,
}

impl Ps2MouseDevice {
    const fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::new()),
        }
    }

    fn attach(&self) -> Result<()> {
        let inner = self.inner.try_lock()?;

        // send next wrote byte to ps/2 secondary port
        PS2_CMD_AND_STATE_REG_ADDR.out8(0xd4);
        inner.wait_ready();

        // init mouse
        PS2_DATA_REG_ADDR.out8(0xff);
        inner.wait_ready();

        PS2_CMD_AND_STATE_REG_ADDR.out8(0xd4);
        inner.wait_ready();

        // start streaming
        PS2_DATA_REG_ADDR.out8(0xf4);
        inner.wait_ready();

        Ok(())
    }
}

impl Device for Ps2MouseDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for Ps2MouseDevice {
    fn read(&self, _offset: usize, _max_len: usize) -> Result<Vec<u8>> {
        Err(Error::NotSupported.into())
    }

    fn write(&self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }
}

impl InterruptSource for Ps2MouseDevice {
    fn handle_irq(&self) {
        let data = PS2_DATA_REG_ADDR.in8();
        if let Ok(mut inner) = self.inner.try_lock() {
            let _ = inner.receive(data);
        }
    }
}

impl Pollable for Ps2MouseDevice {
    fn poll(&self) -> Result<()> {
        loop {
            let event = {
                let mut inner = match self.inner.try_lock() {
                    Ok(inner) => inner,
                    Err(_) => return Ok(()),
                };

                match inner.event() {
                    Ok(Some(e)) => e,
                    Ok(None) => continue,
                    Err(_) => return Ok(()),
                }
            };

            let _ = window_manager::mouse_pointer_event(MouseEvent::Ps2MouseDevice(event));
        }
    }

    fn priority(&self) -> Priority {
        Priority::High
    }
}

pub fn probe_and_attach() -> Result<()> {
    let dev = Arc::new(Ps2MouseDevice::new());
    dev.attach()?;

    vfs::add_dev(dev.clone())?;
    register_irq(VEC_PS2_MOUSE, dev.clone())?;
    register_pollable(dev)?;

    kinfo!("{}: Attached!", NAME);

    Ok(())
}
