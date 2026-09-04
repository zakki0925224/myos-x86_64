use super::{CharDevice, Device, DeviceInfo};
use crate::{
    error::{Error, Result},
    fs::vfs,
    kinfo,
};
use alloc::{sync::Arc, vec::Vec};

const NAME: &str = "zakki";
const MESSAGE: &str = "Hello! I'm Zakki, a low-level programmer!\nCheck out my links below:\n\tX: https://x.com/zakki0925224\n\tGitHub: https://github.com/zakki0925224\n\tPortfolio: https://zakki0925224.github.io\n";

// https://github.com/zakki0925224/zakki_driver
struct ZakkiDevice;

impl Device for ZakkiDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for ZakkiDevice {
    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        kinfo!("{}: Read!", NAME);

        let bytes = MESSAGE.as_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    fn write(&self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }

    fn open(&self) -> Result<()> {
        kinfo!("{}: Opened!", NAME);
        Ok(())
    }

    fn close(&self) -> Result<()> {
        kinfo!("{}: Closed!", NAME);
        Ok(())
    }
}

pub fn probe_and_attach() -> Result<()> {
    vfs::add_dev(Arc::new(ZakkiDevice))?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}
