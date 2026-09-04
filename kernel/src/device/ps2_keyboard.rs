use crate::{
    arch::IoPortAddress,
    device::{
        keyboard, register_irq, register_pollable, CharDevice, Device, DeviceInfo, InterruptSource,
        Pollable,
    },
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util::{
        self,
        fifo::Fifo,
        keyboard::{key_event::*, key_map::*, scan_code::*},
    },
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);
const VEC_PS2_KBD: u8 = 0x21;
const NAME: &str = "ps2-kbd";

struct Inner {
    key_map: KeyMap,
    key_map_cache: Option<BTreeMap<[u8; 6], ScanCode>>,
    mod_keys_state: ModifierKeysState,
    data_buf: Fifo<u8, 128>,
    data: [Option<u8>; 6],
}

impl Inner {
    const fn new(key_map: KeyMap) -> Self {
        Self {
            key_map,
            key_map_cache: None,
            mod_keys_state: ModifierKeysState::default(),
            data_buf: Fifo::new(0),
            data: [None; 6],
        }
    }

    fn input(&mut self, data: u8) -> Result<()> {
        if self.data_buf.enqueue(data).is_err() {
            let _ = self.data_buf.dequeue(); // drop the oldest one only
            self.data_buf.enqueue(data)?;
        }

        Ok(())
    }

    fn event(&mut self) -> Result<Option<KeyEvent>> {
        let byte = self.data_buf.dequeue()?;

        match self.data.iter_mut().find(|d| d.is_none()) {
            Some(slot) => *slot = Some(byte),
            None => {
                self.clear_data();
                self.data[0] = Some(byte);
            }
        }

        let code = self.data.map(|d| d.unwrap_or(0));
        let key_map = self
            .key_map_cache
            .as_ref()
            .ok_or(Error::NotInitialized.with_context("key map cache"))?;

        let complete = key_map.contains_key(&code);
        let e = util::keyboard::key_event_from_ps2(key_map, &mut self.mod_keys_state, code);

        if complete {
            self.clear_data();
        }

        Ok(e)
    }

    fn clear_data(&mut self) {
        self.data.fill(None);
    }

    fn wait_ready(&self) {
        while PS2_CMD_AND_STATE_REG_ADDR.in8() & 0x2 != 0 {
            continue;
        }
    }

    fn attach(&mut self) {
        PS2_CMD_AND_STATE_REG_ADDR.out8(0x60); // write configuration byte
        self.wait_ready();
        PS2_DATA_REG_ADDR.out8(0x47); // enable interrupt
        self.wait_ready();

        self.key_map_cache = Some(self.key_map.to_ps2_map());
    }
}

pub struct Ps2KeyboardDevice {
    inner: Mutex<Inner>,
}

impl Ps2KeyboardDevice {
    const fn new(key_map: KeyMap) -> Self {
        Self {
            inner: Mutex::new(Inner::new(key_map)),
        }
    }
}

impl Device for Ps2KeyboardDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for Ps2KeyboardDevice {
    fn read(&self, _offset: usize, _max_len: usize) -> Result<Vec<u8>> {
        Err(Error::NotSupported.into())
    }

    fn write(&self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }
}

impl InterruptSource for Ps2KeyboardDevice {
    fn handle_irq(&self) {
        let data = PS2_DATA_REG_ADDR.in8();
        if let Ok(mut inner) = self.inner.try_lock() {
            let _ = inner.input(data);
        }
    }
}

impl Pollable for Ps2KeyboardDevice {
    fn poll(&self) -> Result<()> {
        loop {
            let key_event = {
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

            keyboard::push_key_event(key_event)?;
        }
    }
}

pub fn probe_and_attach() -> Result<()> {
    let dev = Arc::new(Ps2KeyboardDevice::new(JIS_JP_109_KEY_MAP));
    dev.inner.try_lock()?.attach();

    vfs::add_dev(dev.clone())?;
    register_irq(VEC_PS2_KBD, dev.clone())?;
    register_pollable(dev)?;

    kinfo!("{}: Attached!", NAME);

    Ok(())
}
