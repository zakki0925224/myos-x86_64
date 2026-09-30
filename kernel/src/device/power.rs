use super::{DeviceInfo, Driver};
use crate::{
    arch::x86_64,
    error::{Error, Result},
    fs::vfs,
    kinfo, kwarn,
    port::qemu,
    sync::mutex::Mutex,
};

const NAME: &str = "power";

static POWER_DRIVER: Mutex<PowerDriver> = Mutex::new(PowerDriver);

struct PowerDriver;

impl Driver for PowerDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        match data.trim_ascii() {
            b"reboot" => x86_64::reboot(),
            b"poweroff" => poweroff(),
            _ => Err(Error::InvalidData.with_context("power command")),
        }
    }
}

fn poweroff() -> Result<()> {
    // try QEMU poweroff
    qemu::poweroff();
    kwarn!("power: Poweroff failed");
    Ok(())
}

pub fn probe_and_attach() -> Result<()> {
    POWER_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&POWER_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}
