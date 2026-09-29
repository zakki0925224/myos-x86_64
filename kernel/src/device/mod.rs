use crate::{error::Error, error::Result, sync::mutex::Mutex};
use alloc::vec::Vec;

pub mod keyboard;
pub mod local_apic_timer;
pub mod panic_screen;
pub mod pci_bus;
pub mod power;
pub mod ps2_keyboard;
pub mod ps2_mouse;
pub mod rtl8139;
pub mod speaker;
pub mod tty;
pub mod uart;
pub mod urandom;
pub mod usb;
pub mod zakki;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: &'static str,
}

impl DeviceInfo {
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

pub trait Driver {
    fn info(&self) -> DeviceInfo;
    fn attach(&mut self) -> Result<()>;

    fn poll(&mut self) -> Result<()> {
        Ok(())
    }

    fn open(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self, _offset: usize, _max_len: usize) -> Result<Vec<u8>> {
        Err(Error::NotSupported.into())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }
}

pub trait CharDevice: Sync {
    fn info(&self) -> Result<DeviceInfo>;
    fn open(&self) -> Result<()>;
    fn close(&self) -> Result<()>;
    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>>;
    fn write(&self, data: &[u8]) -> Result<()>;
}

impl<T: Driver> CharDevice for Mutex<T> {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(self.try_lock()?.info())
    }

    fn open(&self) -> Result<()> {
        self.try_lock()?.open()
    }

    fn close(&self) -> Result<()> {
        self.try_lock()?.close()
    }

    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        self.try_lock()?.read(offset, max_len)
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        self.try_lock()?.write(data)
    }
}
