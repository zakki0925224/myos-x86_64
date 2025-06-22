use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    arch::mmio::Mmio,
    device::{self, pci_bus::conf_space::BaseAddress, xhc::register::*},
    error::{Error, Result},
    fs::vfs,
    info,
    util::mutex::Mutex,
};
use alloc::vec::Vec;

pub mod register;

static mut XHC_DRIVER: Mutex<XhcDriver> = Mutex::new(XhcDriver::new());

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum XhcDriverError {
    InvalidRegisterAddress,
    RegisterNotInitialized,
}

struct XhcDriver {
    device_driver_info: DeviceDriverInfo,
    pci_device_bdf: Option<(usize, usize, usize)>,
    cap_reg: Option<Mmio<CapabilityRegisters>>,
    ope_reg: Option<Mmio<OperationalRegisters>>,
    rt_reg: Option<Mmio<RuntimeRegisters>>,
}

impl XhcDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("xhc"),
            pci_device_bdf: None,
            cap_reg: None,
            ope_reg: None,
            rt_reg: None,
        }
    }

    fn cap_reg(&self) -> Result<&Mmio<CapabilityRegisters>> {
        self.cap_reg
            .as_ref()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn ope_reg(&self) -> Result<&Mmio<OperationalRegisters>> {
        self.ope_reg
            .as_ref()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }

    fn rt_reg(&self) -> Result<&Mmio<RuntimeRegisters>> {
        self.rt_reg
            .as_ref()
            .ok_or(XhcDriverError::RegisterNotInitialized.into())
    }
}

impl DeviceDriverFunction for XhcDriver {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        device::pci_bus::find_devices(0x0c, 0x03, 0x30, |d| {
            self.pci_device_bdf = Some(d.bdf());
            Ok(())
        })?;

        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        if self.pci_device_bdf.is_none() {
            return Err(Error::Failed("Device driver is not probed"));
        }

        let driver_name = self.device_driver_info.name;
        let (bus, device, func) = self.pci_device_bdf.unwrap();
        device::pci_bus::configure_device(bus, device, func, |d| {
            // read base address registers
            let conf_space = d.read_conf_space_non_bridge_field()?;
            let bars = conf_space.get_bars()?;
            if bars.len() == 0 {
                return Err(XhcDriverError::InvalidRegisterAddress.into());
            }

            let cap_reg_virt_addr = match bars[0].1 {
                BaseAddress::MemoryAddress32BitSpace(addr, _) => addr.get_virt_addr()?,
                BaseAddress::MemoryAddress64BitSpace(addr, _) => addr.get_virt_addr()?,
                _ => return Err(XhcDriverError::InvalidRegisterAddress.into()),
            };
            let cap_reg: Mmio<CapabilityRegisters> =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.as_ptr_mut()) };
            let ope_reg_offset = cap_reg.as_ref().cap_reg_len();
            let rt_reg_offset = cap_reg.as_ref().rts_offset();

            self.cap_reg = Some(cap_reg);

            let ope_reg =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.offset(ope_reg_offset).as_ptr_mut()) };
            self.ope_reg = Some(ope_reg);

            let rt_reg =
                unsafe { Mmio::from_raw(cap_reg_virt_addr.offset(rt_reg_offset).as_ptr_mut()) };
            self.rt_reg = Some(rt_reg);

            Ok(())
        })?;

        let dev_desc = vfs::DeviceFileDescriptor {
            get_device_driver_info,
            open,
            close,
            read,
            write,
        };
        vfs::add_dev_file(dev_desc, driver_name)?;
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

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn poll_normal() -> Result<()> {
    let mut driver = unsafe { XHC_DRIVER.try_lock() }?;
    driver.poll_normal()
}
