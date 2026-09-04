use crate::{
    arch::x86_64::{
        apic,
        idt::{self, GateType, InterruptHandler, InterruptStackFrame},
    },
    error::{Error, Result},
    sync::mutex::Mutex,
    task::async_task::Priority,
};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

pub mod keyboard;
pub mod local_apic_timer;
pub mod panic_screen;
pub mod pci_bus;
pub mod ps2_keyboard;
pub mod ps2_mouse;
pub mod rtl8139;
pub mod speaker;
pub mod tty;
pub mod uart;
pub mod urandom;
pub mod usb;
pub mod zakki;

const PIC_VEC_RANGE: core::ops::RangeInclusive<u8> = 0x20..=0x2f;
static IRQ_TABLE: Mutex<BTreeMap<u8, Arc<dyn InterruptSource>>> = Mutex::new(BTreeMap::new());
static POLLABLE_DEVICES: Mutex<Vec<Arc<dyn Pollable>>> = Mutex::new(Vec::new());

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: &'static str,
}

impl DeviceInfo {
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

pub trait Device: Send + Sync {
    fn info(&self) -> Result<DeviceInfo>;

    fn describe(&self) -> Result<String> {
        Ok(self.info()?.name.to_string())
    }
}

pub trait CharDevice: Device {
    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>>;
    fn write(&self, data: &[u8]) -> Result<()>;

    fn open(&self) -> Result<()> {
        Ok(())
    }

    fn close(&self) -> Result<()> {
        Ok(())
    }

    fn can_read(&self) -> bool {
        true
    }
}

pub trait InterruptSource: Send + Sync {
    fn handle_irq(&self);
}

pub trait Pollable: Send + Sync {
    fn poll(&self) -> Result<()>;

    fn priority(&self) -> Priority {
        Priority::Normal
    }
}

macro_rules! irq_stubs {
    ($($name:ident => $vec:expr),* $(,)?) => {
        $(
            extern "x86-interrupt" fn $name(_stack_frame: InterruptStackFrame) {
                dispatch_irq($vec);
            }
        )*

        fn stub_for(vec: u8) -> Result<extern "x86-interrupt" fn(InterruptStackFrame)> {
            match vec {
                $($vec => Ok($name),)*
                _ => Err(Error::NotSupported.with_context("interrupt vector")),
            }
        }
    };
}

irq_stubs! {
    irq_stub_20 => 0x20,
    irq_stub_21 => 0x21,
    irq_stub_22 => 0x22,
    irq_stub_23 => 0x23,
    irq_stub_24 => 0x24,
    irq_stub_25 => 0x25,
    irq_stub_26 => 0x26,
    irq_stub_27 => 0x27,
    irq_stub_28 => 0x28,
    irq_stub_29 => 0x29,
    irq_stub_2a => 0x2a,
    irq_stub_2b => 0x2b,
    irq_stub_2c => 0x2c,
    irq_stub_2d => 0x2d,
    irq_stub_2e => 0x2e,
    irq_stub_2f => 0x2f,
}

fn dispatch_irq(vec: u8) {
    let src = IRQ_TABLE.try_lock().ok().and_then(|t| t.get(&vec).cloned());

    if let Some(src) = src {
        src.handle_irq();
    }

    if PIC_VEC_RANGE.contains(&vec) {
        idt::pic_notify_eoi();
    } else {
        apic::notify_eoi();
    }
}

pub fn register_irq(vec: u8, src: Arc<dyn InterruptSource>) -> Result<()> {
    let stub = stub_for(vec)?;
    idt::set_handler(
        vec as usize,
        InterruptHandler::General(stub),
        GateType::Interrupt,
    )?;
    IRQ_TABLE.try_lock()?.insert(vec, src);

    Ok(())
}

pub fn register_pollable(dev: Arc<dyn Pollable>) -> Result<()> {
    POLLABLE_DEVICES.try_lock()?.push(dev);

    Ok(())
}

pub fn poll_devices(priority: Priority) -> Result<()> {
    let devices: Vec<Arc<dyn Pollable>> = POLLABLE_DEVICES
        .try_lock()?
        .iter()
        .filter(|d| d.priority() == priority)
        .cloned()
        .collect();

    for dev in devices {
        let _ = dev.poll();
    }

    Ok(())
}
