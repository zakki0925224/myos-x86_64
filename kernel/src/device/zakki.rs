use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{error::Result, fs::vfs, kinfo, sync::mutex::Mutex};
use alloc::vec::Vec;

const MESSAGE: &str = "Hello! I'm Zakki, a low-level programmer!\nCheck out my links below:\n\tX: https://x.com/zakki0925224\n\tGitHub: https://github.com/zakki0925224\n\tPortfolio: https://zakki0925224.github.io\n";

static ZAKKI_DRIVER: Mutex<ZakkiDriver> = Mutex::new(ZakkiDriver::new());

// https://github.com/zakki0925224/zakki_driver
struct ZakkiDriver {
    device_driver_info: DeviceDriverInfo,
}

impl ZakkiDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("zakki"),
        }
    }
}

impl DeviceDriverFunction for ZakkiDriver {
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
        kinfo!("{}: Opened!", self.device_driver_info.name);
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        kinfo!("{}: Closed!", self.device_driver_info.name);
        Ok(())
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        kinfo!("{}: Read!", self.device_driver_info.name);
        Ok(MESSAGE.as_bytes().to_vec())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = ZAKKI_DRIVER.try_lock()?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = ZAKKI_DRIVER.try_lock()?;
    driver.probe()?;
    driver.attach(())?;
    kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

fn open() -> Result<()> {
    let mut driver = ZAKKI_DRIVER.try_lock()?;
    driver.open()
}

fn close() -> Result<()> {
    let mut driver = ZAKKI_DRIVER.try_lock()?;
    driver.close()
}

fn read() -> Result<Vec<u8>> {
    let mut driver = ZAKKI_DRIVER.try_lock()?;
    driver.read()
}

fn write(data: &[u8]) -> Result<()> {
    let mut driver = ZAKKI_DRIVER.try_lock()?;
    driver.write(data)
}
