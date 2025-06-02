use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    error::{Error, Result},
    fs::vfs,
    util::mutex::{Mutex, MutexGuard},
};
use alloc::vec::Vec;
use log::info;

const NUM_OF_TTY: usize = 8;
static mut TTYS: [Mutex<Tty>; NUM_OF_TTY] = [
    Mutex::new(Tty::new("tty0", 0)), // kernel console
    Mutex::new(Tty::new("tty1", 1)),
    Mutex::new(Tty::new("tty2", 2)),
    Mutex::new(Tty::new("tty3", 3)),
    Mutex::new(Tty::new("tty4", 4)),
    Mutex::new(Tty::new("tty5", 5)),
    Mutex::new(Tty::new("tty6", 6)),
    Mutex::new(Tty::new("tty7", 7)),
];

struct Tty {
    device_driver_info: DeviceDriverInfo,
    tty_num: usize,
}

impl Tty {
    const fn new(name: &'static str, tty_num: usize) -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new(name),
            tty_num,
        }
    }
}

impl DeviceDriverFunction for Tty {
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
        unimplemented!()
    }

    fn close(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

fn tty_try_lock(num: usize) -> Result<MutexGuard<'static, Tty>> {
    if num >= NUM_OF_TTY {
        return Err(Error::IndexOutOfBoundsError(num));
    }

    unsafe { TTYS[num].try_lock() }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = tty_try_lock(0)?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = tty_try_lock(0)?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

fn open() -> Result<()> {
    let mut driver = tty_try_lock(0)?;
    driver.open()
}

fn close() -> Result<()> {
    let mut driver = tty_try_lock(0)?;
    driver.close()
}

fn read() -> Result<Vec<u8>> {
    let mut driver = tty_try_lock(0)?;
    driver.read()
}

fn write(data: &[u8]) -> Result<()> {
    let mut driver = tty_try_lock(0)?;
    driver.write(data)
}
