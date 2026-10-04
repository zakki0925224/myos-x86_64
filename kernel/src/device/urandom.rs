use crate::{
    device::{self, DeviceInfo, Driver},
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util,
};
use alloc::vec::Vec;

const NAME: &str = "urandom";

static URANDOM_DRIVER: Mutex<UrandomDriver> = Mutex::new(UrandomDriver);

struct UrandomDriver;

impl Driver for UrandomDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self, _offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let uptime_durtion = device::local_apic_timer::global_uptime();
        let seed = uptime_durtion.as_nanos() as u64;
        Ok(util::random::random_bytes_pcg32(max_len, seed))
    }
}

pub fn probe_and_attach() -> Result<()> {
    URANDOM_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&URANDOM_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}
