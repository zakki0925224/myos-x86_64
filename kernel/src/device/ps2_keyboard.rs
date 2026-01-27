use crate::{
    arch::{
        x86_64::{self, idt},
        IoPortAddress,
    },
    device::{tty, DeviceDriverFunction, DeviceDriverInfo},
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util::{
        self,
        fifo::Fifo,
        keyboard::{key_event::*, key_map::*, scan_code::*},
    },
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);

static PS2_KBD_DRIVER: Mutex<Ps2KeyboardDriver> =
    Mutex::new(Ps2KeyboardDriver::new(ANSI_US_104_KEY_MAP));

struct Ps2KeyboardDriver {
    device_driver_info: DeviceDriverInfo,
    key_map: KeyMap,
    key_map_cache: Option<BTreeMap<[u8; 6], ScanCode>>,
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
            key_map_cache: None,
            mod_keys_state: ModifierKeysState::default(),
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

        let e = util::keyboard::get_key_event_from_ps2(
            self.key_map_cache.as_ref().unwrap(),
            &mut self.mod_keys_state,
            code,
        );
        if e.is_some() {
            self.clear_data();
        }

        Ok(e)
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

        self.key_map_cache = Some(self.key_map.to_ps2_map());

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
    let driver = PS2_KBD_DRIVER.try_lock()?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    x86_64::disabled_int(|| {
        let mut driver = PS2_KBD_DRIVER.try_lock()?;
        driver.probe()?;
        driver.attach(())?;
        kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);
        Ok(())
    })
}

pub fn open() -> Result<()> {
    let mut driver = PS2_KBD_DRIVER.try_lock()?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = PS2_KBD_DRIVER.try_lock()?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = PS2_KBD_DRIVER.try_lock()?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = PS2_KBD_DRIVER.try_lock()?;
    driver.write(data)
}

pub fn poll_normal() -> Result<()> {
    let key_event = x86_64::disabled_int(|| {
        let mut driver = PS2_KBD_DRIVER.try_lock()?;
        driver.poll_normal()
    })?;
    let key_event = match key_event {
        Some(e) => e,
        None => return Ok(()),
    };

    match key_event.code {
        KeyCode::CursorUp => {
            tty::input('\x1b')?;
            tty::input('[')?;
            tty::input('A')?;
            return Ok(());
        }
        KeyCode::CursorDown => {
            tty::input('\x1b')?;
            tty::input('[')?;
            tty::input('B')?;
            return Ok(());
        }
        KeyCode::CursorRight => {
            tty::input('\x1b')?;
            tty::input('[')?;
            tty::input('C')?;
            return Ok(());
        }
        KeyCode::CursorLeft => {
            tty::input('\x1b')?;
            tty::input('[')?;
            tty::input('D')?;
            return Ok(());
        }
        _ => (),
    }

    let c = match key_event.c {
        Some(c) => c,
        None => return Ok(()),
    };

    tty::input(c)
}

pub extern "x86-interrupt" fn poll_int_ps2_kbd_driver(_stack_frame: idt::InterruptStackFrame) {
    if let Ok(mut driver) = PS2_KBD_DRIVER.try_lock() {
        let _ = driver.poll_int();
    }
    idt::notify_end_of_int();
}
