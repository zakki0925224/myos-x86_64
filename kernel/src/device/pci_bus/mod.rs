use super::{CharDevice, Device, DeviceInfo};
use crate::{
    error::{Error, Result},
    fs::vfs,
    kdebug, kerror, kinfo,
    sync::mutex::Mutex,
};
use alloc::{string::String, sync::Arc, vec::Vec};
use conf_space::*;
use device::PciDevice;

pub mod conf_space;
pub mod device;

const NAME: &str = "pci-bus";

static PCI_BUS: Mutex<PciBus> = Mutex::new(PciBus::new());
static PCI_DRIVERS: Mutex<Vec<Arc<dyn PciDriver>>> = Mutex::new(Vec::new());

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

struct PciBus {
    device_info: DeviceInfo,
    pci_devices: Vec<PciDevice>,
}

impl PciBus {
    const fn new() -> Self {
        Self {
            device_info: DeviceInfo::new("pci-bus"),
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
                        self.device_info.name,
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

impl PciBus {
    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self) -> Result<()> {
        vfs::add_dev(Arc::new(PciBusDevice))?;
        Ok(())
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
            s.push_str(&d.describe()?);
        }

        let bytes = s.into_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        Err(Error::NotSupported.into())
    }
}

pub trait PciDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self, dev: &PciDevice) -> Result<bool>;
}

pub fn register_driver(driver: Arc<dyn PciDriver>) -> Result<()> {
    PCI_DRIVERS.try_lock()?.push(driver);

    Ok(())
}

pub fn probe_all() -> Result<()> {
    let devices: Vec<PciDevice> = PCI_BUS.try_lock()?.pci_devices.clone();
    let drivers: Vec<Arc<dyn PciDriver>> = PCI_DRIVERS.try_lock()?.clone();

    for device in devices {
        let (bus, dev, func) = device.bdf();

        for driver in &drivers {
            match driver.probe(&device) {
                Ok(false) => continue,
                Ok(true) => {
                    kinfo!(
                        "pci-bus: {} attached to {}:{}:{}",
                        driver.name(),
                        bus,
                        dev,
                        func
                    );
                    break;
                }
                Err(err) => {
                    kerror!(
                        "pci-bus: {}: Failed to probe {}:{}:{}: {:?}",
                        driver.name(),
                        bus,
                        dev,
                        func,
                        err
                    );
                    break;
                }
            }
        }
    }

    Ok(())
}

pub fn device_info() -> Result<DeviceInfo> {
    Ok(DeviceInfo::new(NAME))
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = PCI_BUS.try_lock()?;

    driver.probe()?;
    driver.attach()?;
    kinfo!("{}: Attached!", NAME);

    kinfo!("{}: Scanning devices...", NAME);
    driver.scan_pci_devices();
    Ok(())
}

struct PciBusDevice;

impl Device for PciBusDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for PciBusDevice {
    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        PCI_BUS.try_lock()?.read(offset, max_len)
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        PCI_BUS.try_lock()?.write(data)
    }

    fn open(&self) -> Result<()> {
        PCI_BUS.try_lock()?.open()
    }

    fn close(&self) -> Result<()> {
        PCI_BUS.try_lock()?.close()
    }
}
