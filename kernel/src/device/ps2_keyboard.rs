use crate::{
    arch::{
        x86_64::idt::{self, GateType, InterruptHandler, InterruptStackFrame},
        IoPortAddress,
    },
    device::{keyboard, DeviceInfo, Driver},
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
use alloc::collections::btree_map::BTreeMap;

const PS2_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0x60);
const PS2_CMD_AND_STATE_REG_ADDR: IoPortAddress = IoPortAddress::new(0x64);
const VEC_PS2_KBD: u8 = 0x21;
const NAME: &str = "ps2-kbd";

static PS2_KEYBOARD_DRIVER: Mutex<Ps2KeyboardDriver> =
    Mutex::new(Ps2KeyboardDriver::new(JIS_JP_109_KEY_MAP));

struct Ps2KeyboardDriver {
    key_map: KeyMap,
    key_map_cache: Option<BTreeMap<[u8; 6], ScanCode>>,
    mod_keys_state: ModifierKeysState,
    data_buf: Fifo<u8, 128>,
    data: [Option<u8>; 6],
}

impl Ps2KeyboardDriver {
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
}

impl Driver for Ps2KeyboardDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        PS2_CMD_AND_STATE_REG_ADDR.out8(0x60); // write configuration byte
        self.wait_ready();
        PS2_DATA_REG_ADDR.out8(0x47); // enable interrupt
        self.wait_ready();

        self.key_map_cache = Some(self.key_map.to_ps2_map());

        idt::set_handler(
            VEC_PS2_KBD as usize,
            InterruptHandler::General(ps2_kbd_isr),
            GateType::Interrupt,
        )?;

        Ok(())
    }

    fn poll(&mut self) -> Result<()> {
        loop {
            match self.event() {
                Ok(Some(e)) => keyboard::push_key_event(e)?,
                Ok(None) => continue,
                Err(_) => return Ok(()),
            }
        }
    }
}

extern "x86-interrupt" fn ps2_kbd_isr(_stack_frame: InterruptStackFrame) {
    let data = PS2_DATA_REG_ADDR.in8();
    if let Ok(mut driver) = PS2_KEYBOARD_DRIVER.try_lock() {
        let _ = driver.input(data);
    }
    idt::pic_notify_eoi();
}

pub fn probe_and_attach() -> Result<()> {
    PS2_KEYBOARD_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&PS2_KEYBOARD_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn poll_normal() -> Result<()> {
    PS2_KEYBOARD_DRIVER.try_lock()?.poll()
}
