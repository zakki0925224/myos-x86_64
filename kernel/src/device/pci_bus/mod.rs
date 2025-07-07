use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    debug_,
    error::{Error, Result},
    fs::vfs,
    info,
    sync::mutex::Mutex,
};
use alloc::{string::String, vec::Vec};
use conf_space::*;
use device::{PciDevice, PciDeviceFunction};

pub mod conf_space;
mod device;

static mut PCI_BUS_DRIVER: Mutex<PciBusDriver> = Mutex::new(PciBusDriver::new());

struct PciBusDriver {
    device_driver_info: DeviceDriverInfo,
    pci_devices: Vec<PciDevice>,
}

impl PciBusDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("pci-bus"),
            pci_devices: Vec::new(),
        }
    }

    fn scan_pci_devices(&mut self) {
        let mut devices = Vec::new();

        'b: for bus in 0..PCI_DEVICE_BUS_LEN {
            for device in 0..PCI_DEVICE_DEVICE_LEN {
                for func in 0..PCI_DEVICE_FUNC_LEN {
                    let pci_device = match PciDevice::try_new(bus, device, func) {
                        Some(dev) => dev,
                        None => {
                            if func == 0 {
                                continue 'b;
                            } else {
                                continue;
                            }
                        }
                    };

                    debug_!(
                        "{}: {}.{}.{} {} found",
                        self.device_driver_info.name,
                        bus,
                        device,
                        func,
                        pci_device
                            .read_conf_space_header()
                            .unwrap()
                            .get_device_name()
                            .unwrap_or("<UNKNOWN NAME>")
                    );
                    devices.push(pci_device);
                }
            }
        }

        self.pci_devices = devices;
    }

    fn find_device(&self, bus: usize, device: usize, func: usize) -> Result<&PciDevice> {
        self.pci_devices
            .iter()
            .find(|d| d.bdf() == (bus, device, func))
            .ok_or(Error::Failed("PCI device not found"))
    }

    fn find_device_mut(
        &mut self,
        bus: usize,
        device: usize,
        func: usize,
    ) -> Result<&mut PciDevice> {
        self.pci_devices
            .iter_mut()
            .find(|d| d.bdf() == (bus, device, func))
            .ok_or(Error::Failed("PCI device not found"))
    }

    fn find_devices_by_class_mut(
        &mut self,
        class: u8,
        subclass: u8,
        prog_if: u8,
    ) -> Vec<&mut PciDevice> {
        self.pci_devices
            .iter_mut()
            .filter(|d| d.device_class() == (class, subclass, prog_if))
            .collect()
    }

    fn find_device_by_vendor_and_device_id_mut(
        &mut self,
        vendor_id: u16,
        device_id: u16,
    ) -> Result<&mut PciDevice> {
        self.pci_devices
            .iter_mut()
            .find(|d| {
                let conf_space_header = d.read_conf_space_header().unwrap();
                conf_space_header.vendor_id == vendor_id && conf_space_header.device_id == device_id
            })
            .ok_or(Error::Failed("PCI device not found"))
    }
}

impl DeviceDriverFunction for PciBusDriver {
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
        let mut s = String::new();

        for d in &self.pci_devices {
            let (bus, device, func) = d.bdf();
            let conf_space_header = d.read_conf_space_header().unwrap();
            let header_type = conf_space_header.get_header_type();
            let device_name = conf_space_header
                .get_device_name()
                .unwrap_or("<UNKNOWN NAME>");

            s.push_str(&format!("{}:{}:{}", bus, device, func));
            s.push_str(&format!(" {:?} - {}\n", header_type, device_name));
        }

        Ok(s.into_bytes())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    unsafe { PCI_BUS_DRIVER.try_lock()? }.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { PCI_BUS_DRIVER.try_lock() }?;
    let driver_name = driver.get_device_driver_info()?.name;

    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver_name);

    info!("{}: Scanning devices...", driver_name);
    driver.scan_pci_devices();
    Ok(())
}

pub fn open() -> Result<()> {
    unsafe { PCI_BUS_DRIVER.try_lock() }?.open()
}

pub fn close() -> Result<()> {
    unsafe { PCI_BUS_DRIVER.try_lock() }?.close()
}

pub fn read() -> Result<Vec<u8>> {
    unsafe { PCI_BUS_DRIVER.try_lock() }?.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    unsafe { PCI_BUS_DRIVER.try_lock() }?.write(data)
}

pub fn is_exist_device(bus: usize, device: usize, func: usize) -> Result<bool> {
    let is_exist = unsafe { PCI_BUS_DRIVER.try_lock() }?
        .find_device(bus, device, func)
        .is_ok();
    Ok(is_exist)
}

pub fn configure_device<F: FnMut(&mut dyn PciDeviceFunction) -> Result<()>>(
    bus: usize,
    device: usize,
    func: usize,
    mut f: F,
) -> Result<()> {
    let mut driver = unsafe { PCI_BUS_DRIVER.try_lock() }?;
    let device_mut = driver.find_device_mut(bus, device, func)?;

    f(device_mut)
}

pub fn find_devices<F: FnMut(&mut dyn PciDeviceFunction) -> Result<()>>(
    class: u8,
    subclass: u8,
    prog_if: u8,
    mut f: F,
) -> Result<()> {
    let mut driver = unsafe { PCI_BUS_DRIVER.try_lock() }?;
    let devices = driver.find_devices_by_class_mut(class, subclass, prog_if);

    for device in devices {
        f(device)?;
    }

    Ok(())
}

pub fn find_device_by_vendor_and_device_id<F: FnMut(&mut dyn PciDeviceFunction) -> Result<()>>(
    vendor_id: u16,
    device_id: u16,
    mut f: F,
) -> Result<()> {
    let mut driver = unsafe { PCI_BUS_DRIVER.try_lock() }?;
    let device = driver.find_device_by_vendor_and_device_id_mut(vendor_id, device_id)?;

    f(device)
}
