use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{error::Result, fs::vfs, kdebug, kinfo, sync::mutex::Mutex};
use alloc::{string::String, vec::Vec};
use conf_space::*;
use device::{PciDevice, PciDeviceFunction};

pub mod conf_space;
mod device;

static PCI_BUS_DRIVER: Mutex<PciBusDriver> = Mutex::new(PciBusDriver::new());

#[derive(Debug)]
pub enum PciError {
    DeviceNotFoundByBdf {
        bus: usize,
        device: usize,
        func: usize,
    },
    DeviceNotFoundById {
        vendor_id: u16,
        device_id: u16,
    },
    InvalidConfigurationSpaceHeaderType(ConfigurationSpaceHeaderType),
    FailedToReadMsiCapabilityFields,
    MsiCapabilityFieldWasNotFound,
}

impl core::fmt::Display for PciError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeviceNotFoundByBdf { bus, device, func } => {
                write!(f, "Device not found: {:#x}:{:#x}:{:#x}", bus, device, func)
            }
            Self::DeviceNotFoundById {
                vendor_id,
                device_id,
            } => write!(
                f,
                "Device not found: vendor: {:#x}, device: {:#x}",
                vendor_id, device_id
            ),
            Self::InvalidConfigurationSpaceHeaderType(header_type) => write!(
                f,
                "Invalid configuration space header type: {:?}",
                header_type
            ),
            Self::FailedToReadMsiCapabilityFields => {
                write!(f, "Failed to read MSI capability fields")
            }
            Self::MsiCapabilityFieldWasNotFound => write!(f, "MSI capability field was not found"),
        }
    }
}

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

                    kdebug!(
                        "{}: {}.{}.{} {} found",
                        self.device_driver_info.name,
                        bus,
                        device,
                        func,
                        pci_device
                            .read_conf_space_header()
                            .unwrap()
                            .device_name()
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
            .ok_or(PciError::DeviceNotFoundByBdf { bus, device, func }.into())
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
            .ok_or(PciError::DeviceNotFoundByBdf { bus, device, func }.into())
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
            .ok_or(
                PciError::DeviceNotFoundById {
                    vendor_id,
                    device_id,
                }
                .into(),
            )
    }
}

impl DeviceDriverFunction for PciBusDriver {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        let dev_desc = vfs::DeviceFileDescriptor {
            device_driver_info,
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

    fn read(&mut self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let mut s = String::new();

        for d in &self.pci_devices {
            let (bus, device, func) = d.bdf();
            let conf_space_header = d.read_conf_space_header().unwrap();
            let header_type = conf_space_header.header_type();
            let device_name = conf_space_header.device_name().unwrap_or("<UNKNOWN NAME>");

            s.push_str(&format!("{}:{}:{}", bus, device, func));
            s.push_str(&format!(" {:?} - {}\n", header_type, device_name));
        }

        let bytes = s.into_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn device_driver_info() -> Result<DeviceDriverInfo> {
    PCI_BUS_DRIVER.try_lock()?.device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = PCI_BUS_DRIVER.try_lock()?;
    let driver_name = driver.device_driver_info()?.name;

    driver.probe()?;
    driver.attach(())?;
    kinfo!("{}: Attached!", driver_name);

    kinfo!("{}: Scanning devices...", driver_name);
    driver.scan_pci_devices();
    Ok(())
}

pub fn open() -> Result<()> {
    PCI_BUS_DRIVER.try_lock()?.open()
}

pub fn close() -> Result<()> {
    PCI_BUS_DRIVER.try_lock()?.close()
}

pub fn read(offset: usize, max_len: usize) -> Result<Vec<u8>> {
    PCI_BUS_DRIVER.try_lock()?.read(offset, max_len)
}

pub fn write(data: &[u8]) -> Result<()> {
    PCI_BUS_DRIVER.try_lock()?.write(data)
}

pub fn device_exists(bus: usize, device: usize, func: usize) -> Result<bool> {
    let exists = PCI_BUS_DRIVER
        .try_lock()?
        .find_device(bus, device, func)
        .is_ok();
    Ok(exists)
}

pub fn configure_device<F: FnMut(&mut dyn PciDeviceFunction) -> Result<()>>(
    bus: usize,
    device: usize,
    func: usize,
    mut f: F,
) -> Result<()> {
    let mut driver = PCI_BUS_DRIVER.try_lock()?;
    let device_mut = driver.find_device_mut(bus, device, func)?;

    f(device_mut)
}

pub fn find_devices<F: FnMut(&mut dyn PciDeviceFunction) -> Result<()>>(
    class: u8,
    subclass: u8,
    prog_if: u8,
    mut f: F,
) -> Result<()> {
    let mut driver = PCI_BUS_DRIVER.try_lock()?;
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
    let mut driver = PCI_BUS_DRIVER.try_lock()?;
    let device = driver.find_device_by_vendor_and_device_id_mut(vendor_id, device_id)?;

    f(device)
}
