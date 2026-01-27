use crate::{
    arch::{
        x86_64::{self, idt},
        IoPortAddress,
    },
    device::{DeviceDriverFunction, DeviceDriverInfo},
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util::fifo::Fifo,
};
use alloc::vec::Vec;

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);

static PS2_MOUSE_DRIVER: Mutex<Ps2MouseDriver> = Mutex::new(Ps2MouseDriver::new());

#[derive(Default, Debug)]
pub struct Ps2MouseEvent {
    pub middle: bool,
    pub right: bool,
    pub left: bool,
    pub rel_x: i16,
    pub rel_y: i16,
}

enum Ps2MousePhase {
    WaitingAck,
    WaitingData0,
    WaitingData1,
    WaitingData2,
}

impl Ps2MousePhase {
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

struct Ps2MouseDriver {
    device_driver_info: DeviceDriverInfo,
    mouse_phase: Ps2MousePhase,
    data_buf: Fifo<u8, 256>,
    data_buf2: [u8; 3],
}

impl Ps2MouseDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("ps2-mouse"),
            mouse_phase: Ps2MousePhase::default(),
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

    fn get_event(&mut self) -> Result<Option<Ps2MouseEvent>> {
        let data = self.data_buf.dequeue()?;
        let e = match self.mouse_phase {
            Ps2MousePhase::WaitingAck => {
                if data == 0xfa {
                    self.mouse_phase.next();
                }

                None
            }
            Ps2MousePhase::WaitingData0 => {
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
            Ps2MousePhase::WaitingData1 => {
                self.data_buf2[1] = data;
                self.mouse_phase.next();
                None
            }
            Ps2MousePhase::WaitingData2 => {
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

impl DeviceDriverFunction for Ps2MouseDriver {
    type AttachInput = ();
    type PollNormalOutput = Option<Ps2MouseEvent>;
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
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
        if !self.device_driver_info.attached {
            return Err("Device driver is not attached".into());
        }

        self.get_event()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        if !self.device_driver_info.attached {
            return Err("Device driver is not attached".into());
        }

        let data = PS2_DATA_REG_ADDR.in8();
        self.receive(data)?;

        Ok(())
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

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = PS2_MOUSE_DRIVER.try_lock()?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    x86_64::disabled_int(|| {
        let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
        driver.probe()?;
        driver.attach(())?;
        kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);
        Ok(())
    })
}

pub fn open() -> Result<()> {
    let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
    driver.write(data)
}

pub fn poll_normal() -> Result<Option<Ps2MouseEvent>> {
    x86_64::disabled_int(|| {
        let mut driver = PS2_MOUSE_DRIVER.try_lock()?;
        driver.poll_normal()
    })
}

pub extern "x86-interrupt" fn poll_int_ps2_mouse_driver(_stack_frame: idt::InterruptStackFrame) {
    if let Ok(mut driver) = PS2_MOUSE_DRIVER.try_lock() {
        let _ = driver.poll_int();
    }
    idt::notify_end_of_int();
}
