use crate::{
    device::{DeviceInfo, Driver},
    error::Result,
    fs::vfs,
    kdebug, kinfo,
    sync::mutex::Mutex,
};
use alloc::{string::String, vec::Vec};
use conf_space::*;
use device::PciDevice;

pub mod conf_space;
pub mod device;

const NAME: &str = "pci-bus";

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
    pci_devices: Vec<PciDevice>,
}

impl PciBusDriver {
    const fn new() -> Self {
        Self {
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
                        NAME,
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
}

impl Driver for PciBusDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        kinfo!("{}: Scanning devices...", NAME);
        self.scan_pci_devices();
        Ok(())
    }

    fn read(&mut self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let mut s = String::new();

        for d in &self.pci_devices {
            s.push_str(&d.describe()?);
        }

        let bytes = s.into_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

pub fn probe_and_attach() -> Result<()> {
    PCI_BUS_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&PCI_BUS_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn find_device_by_class(class: (u8, u8, u8)) -> Result<Option<PciDevice>> {
    let driver = PCI_BUS_DRIVER.try_lock()?;
    Ok(driver
        .pci_devices
        .iter()
        .find(|d| d.device_class() == class)
        .cloned())
}

pub fn find_device_by_id(vendor_id: u16, device_id: u16) -> Result<Option<PciDevice>> {
    let driver = PCI_BUS_DRIVER.try_lock()?;

    for d in &driver.pci_devices {
        let header = d.read_conf_space_header()?;
        if (header.vendor_id, header.device_id) == (vendor_id, device_id) {
            return Ok(Some(d.clone()));
        }
    }

    Ok(None)
}
