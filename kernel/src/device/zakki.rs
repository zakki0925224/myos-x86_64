use super::{DeviceInfo, Driver};
use crate::{error::Result, fs::vfs, kinfo, sync::mutex::Mutex};
use alloc::vec::Vec;

const NAME: &str = "zakki";
const MESSAGE: &str = "Hello! I'm Zakki, a low-level programmer!\nCheck out my links below:\n\tX: https://x.com/zakki0925224\n\tGitHub: https://github.com/zakki0925224\n\tPortfolio: https://zakki0925224.github.io\n";

static ZAKKI_DRIVER: Mutex<ZakkiDriver> = Mutex::new(ZakkiDriver);

// https://github.com/zakki0925224/zakki_driver
struct ZakkiDriver;

impl Driver for ZakkiDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let bytes = MESSAGE.as_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

pub fn probe_and_attach() -> Result<()> {
    ZAKKI_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&ZAKKI_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}
