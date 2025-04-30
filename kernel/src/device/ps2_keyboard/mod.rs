use self::{key_event::KeyEvent, key_map::KeyMap};
use super::{console, DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    arch::{self, addr::IoPortAddress},
    device::ps2_keyboard::{
        key_event::{KeyState, ModifierKeysState},
        key_map::ANSI_US_104_KEY_MAP,
    },
    error::{Error, Result},
    fs::vfs,
    idt,
    util::{fifo::Fifo, mutex::Mutex},
};
use alloc::vec::Vec;
use log::info;

pub mod key_event;
mod key_map;
mod scan_code;

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);

static mut PS2_KBD_DRIVER: Mutex<Ps2KeyboardDriver> =
    Mutex::new(Ps2KeyboardDriver::new(ANSI_US_104_KEY_MAP));

struct Ps2KeyboardDriver {
    device_driver_info: DeviceDriverInfo,
    key_map: KeyMap,
    mod_keys_state: ModifierKeysState,
    data_buf: Fifo<u8, 128>,
    data_0: Option<u8>,
    data_1: Option<u8>,
    data_2: Option<u8>,
    data_3: Option<u8>,
    data_4: Option<u8>,
    data_5: Option<u8>,
}

impl Ps2KeyboardDriver {
    const fn new(key_map: KeyMap) -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("ps2-kbd"),
            key_map,
            mod_keys_state: ModifierKeysState {
                shift: false,
                ctrl: false,
                gui: false,
                alt: false,
            },
            data_buf: Fifo::new(0),
            data_0: None,
            data_1: None,
            data_2: None,
            data_3: None,
            data_4: None,
            data_5: None,
        }
    }

    fn input(&mut self, data: u8) -> Result<()> {
        if self.data_buf.enqueue(data).is_err() {
            self.data_buf.reset_ptr();
            self.data_buf.enqueue(data)?;
        }

        //println!("{:?}", self.data_buf.get_buf_ref());

        Ok(())
    }

    fn get_event(&mut self) -> Result<Option<KeyEvent>> {
        let data = self.data_buf.dequeue()?;

        if self.data_0.is_none() {
            self.data_0 = Some(data);
        } else if self.data_1.is_none() {
            self.data_1 = Some(data);
        } else if self.data_2.is_none() {
            self.data_2 = Some(data);
        } else if self.data_3.is_none() {
            self.data_3 = Some(data);
        } else if self.data_4.is_none() {
            self.data_4 = Some(data);
        } else if self.data_5.is_none() {
            self.data_5 = Some(data);
        } else {
            self.clear_data();
            self.data_0 = Some(data);
        }

        let code = [
            self.data_0.unwrap_or(0),
            self.data_1.unwrap_or(0),
            self.data_2.unwrap_or(0),
            self.data_3.unwrap_or(0),
            self.data_4.unwrap_or(0),
            self.data_5.unwrap_or(0),
        ];
        let key_map = match self.key_map {
            KeyMap::AnsiUs104(map) => map,
        };

        for scan_code in key_map {
            let key_code = scan_code.key_code;

            let key_state = match scan_code {
                sc if sc.pressed == code => KeyState::Pressed,
                sc if sc.released == code => KeyState::Released,
                _ => continue,
            };

            // prev keys
            let ModifierKeysState {
                shift: mut shift,
                ctrl: mut ctrl,
                gui: mut gui,
                alt: mut alt,
            } = self.mod_keys_state;

            if key_code.is_shift() {
                shift = key_state == KeyState::Pressed;
            } else if key_code.is_ctrl() {
                ctrl = key_state == KeyState::Pressed;
            } else if key_code.is_gui() {
                gui = key_state == KeyState::Pressed;
            } else if key_code.is_alt() {
                alt = key_state == KeyState::Pressed;
            }

            self.mod_keys_state = ModifierKeysState {
                shift,
                ctrl,
                gui,
                alt,
            };

            let mut c = match self.mod_keys_state.shift {
                true => scan_code.on_shift_c,
                false => scan_code.c,
            };

            if c.is_some() && self.mod_keys_state.ctrl {
                match c.unwrap() as u8 {
                    0x40..=0x5f => {
                        c = Some((c.unwrap() as u8 - 0x40) as char);
                    }
                    0x60..=0x7f => {
                        c = Some((c.unwrap() as u8 - 0x60) as char);
                    }
                    _ => (),
                }
            }

            let key_event = KeyEvent {
                code: key_code,
                state: key_state,
                c,
            };

            self.clear_data();
            return Ok(Some(key_event));
        }

        Ok(None)
    }

    fn clear_data(&mut self) {
        self.data_0 = None;
        self.data_1 = None;
        self.data_2 = None;
        self.data_3 = None;
        self.data_4 = None;
        self.data_5 = None;
    }

    fn wait_ready(&self) {
        while PS2_CMD_AND_STATE_REG_ADDR.in8() & 0x2 != 0 {
            continue;
        }
    }
}

impl DeviceDriverFunction for Ps2KeyboardDriver {
    type AttachInput = ();
    type PollNormalOutput = Option<KeyEvent>;
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        PS2_CMD_AND_STATE_REG_ADDR.out8(0x60); // write configuration byte
        self.wait_ready();
        PS2_DATA_REG_ADDR.out8(0x47); // enable interrupt
        self.wait_ready();

        PS2_CMD_AND_STATE_REG_ADDR.out8(0x20); // read configuration byte
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
            return Err(Error::Failed("Device driver is not attached"));
        }

        self.get_event()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        if !self.device_driver_info.attached {
            return Err(Error::Failed("Device driver is not attached"));
        }

        let data = PS2_DATA_REG_ADDR.in8();
        self.input(data)?;

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
    let driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    arch::disabled_int(|| {
        let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
        driver.probe()?;
        driver.attach(())?;
        info!("{}: Attached!", driver.get_device_driver_info()?.name);
        Ok(())
    })
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn poll_normal() -> Result<()> {
    let key_event = arch::disabled_int(|| {
        let mut driver = unsafe { PS2_KBD_DRIVER.try_lock() }?;
        driver.poll_normal()
    })?;
    let key_event = match key_event {
        Some(e) => e,
        None => return Ok(()),
    };

    if key_event.state == KeyState::Released {
        return Ok(());
    }

    let c = match key_event.c {
        Some(c) => c,
        None => return Ok(()),
    };

    console::input(c)
}

pub extern "x86-interrupt" fn poll_int_ps2_kbd_driver() {
    if let Ok(mut driver) = unsafe { PS2_KBD_DRIVER.try_lock() } {
        let _ = driver.poll_int();
    }
    idt::notify_end_of_int();
}
