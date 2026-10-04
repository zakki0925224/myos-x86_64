use crate::{
    arch::{
        x86_64::idt::{self, GateType, InterruptHandler, InterruptStackFrame},
        IoPortAddress,
    },
    device::{mouse, DeviceInfo, Driver},
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util::{fifo::Fifo, mouse::mouse_event::*},
};

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);

const VEC_PS2_MOUSE: u8 = 0x2c;
const NAME: &str = "ps2-mouse";

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

static PS2_MOUSE_DRIVER: Mutex<Ps2MouseDriver> = Mutex::new(Ps2MouseDriver::new());

struct Ps2MouseDriver {
    mouse_phase: MousePhase,
    data_buf: Fifo<u8, 256>,
    data_buf2: [u8; 3],
}

impl Ps2MouseDriver {
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

impl Driver for Ps2MouseDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        // send next wrote byte to ps/2 secondary port
        PS2_CMD_AND_STATE_REG_ADDR.out8(0xd4);
        self.wait_ready();

        // init mouse
        PS2_DATA_REG_ADDR.out8(0xff);
        self.wait_ready();

        PS2_CMD_AND_STATE_REG_ADDR.out8(0xd4);
        self.wait_ready();

        // start streaming
        PS2_DATA_REG_ADDR.out8(0xf4);
        self.wait_ready();

        idt::set_handler(
            VEC_PS2_MOUSE as usize,
            InterruptHandler::General(ps2_mouse_isr),
            GateType::Interrupt,
        )?;

        Ok(())
    }

    fn poll(&mut self) -> Result<()> {
        loop {
            match self.event() {
                Ok(Some(e)) => {
                    let _ = mouse::push_event(MouseEvent::Ps2MouseDevice(e));
                }
                Ok(None) => continue,
                Err(_) => return Ok(()),
            }
        }
    }
}

extern "x86-interrupt" fn ps2_mouse_isr(_stack_frame: InterruptStackFrame) {
    let data = PS2_DATA_REG_ADDR.in8();
    if let Ok(mut driver) = PS2_MOUSE_DRIVER.try_lock() {
        let _ = driver.receive(data);
    }
    idt::pic_notify_eoi();
}

pub fn probe_and_attach() -> Result<()> {
    PS2_MOUSE_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&PS2_MOUSE_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn poll_normal() -> Result<()> {
    PS2_MOUSE_DRIVER.try_lock()?.poll()
}
