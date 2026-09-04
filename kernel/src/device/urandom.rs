use crate::{
    device::{self, CharDevice, Device, DeviceInfo},
    error::{Error, Result},
    fs::vfs,
    kinfo, util,
};
use alloc::{sync::Arc, vec::Vec};

const NAME: &str = "urandom";

struct UrandomDevice;

impl Device for UrandomDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for UrandomDevice {
    fn read(&self, _offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let uptime_durtion = device::local_apic_timer::global_uptime();
        let seed = uptime_durtion.as_nanos() as u64;
        Ok(util::random::random_bytes_pcg32(max_len, seed))
    }

    fn write(&self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }
}

pub fn probe_and_attach() -> Result<()> {
    vfs::add_dev(Arc::new(UrandomDevice))?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}
