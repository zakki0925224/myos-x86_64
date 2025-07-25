use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{device, error::Result, fs::vfs, info, sync::mutex::Mutex, util};
use alloc::vec::Vec;

static mut URANDOM_DRIVER: Mutex<UrandomDriver> = Mutex::new(UrandomDriver::new());

struct UrandomDriver {
    device_driver_info: DeviceDriverInfo,
}

impl UrandomDriver {
    const DEFAULT_SIZE: usize = 256;

    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("urandom"),
        }
    }
}

impl DeviceDriverFunction for UrandomDriver {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
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
        unimplemented!()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        unimplemented!()
    }

    fn open(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        let uptime_durtion = device::local_apic_timer::global_uptime();
        let seed = uptime_durtion.as_nanos() as u64;
        let buf = util::random::random_bytes_pcg32(Self::DEFAULT_SIZE, seed);
        Ok(buf)
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        Ok(())
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);

    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { URANDOM_DRIVER.try_lock() }?;
    driver.write(data)
}
