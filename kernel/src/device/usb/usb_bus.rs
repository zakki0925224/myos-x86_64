use crate::{
    device::{
        usb::{
            xhc::{desc::*, register::*},
            UsbDriver, UsbHostController,
        },
        CharDevice, Device, DeviceInfo,
    },
    error::{Error, Result},
    fs::vfs,
    kerror, kinfo,
    sync::mutex::{Mutex, MutexGuard},
};
use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

const NAME: &str = "usb-bus";

static USB_BUS: Mutex<UsbBus> = Mutex::new(UsbBus::new());
static USB_DRIVERS: Mutex<Vec<Arc<dyn UsbDriver>>> = Mutex::new(Vec::new());

pub struct XhciAttachInfo {
    pub port: usize,
    pub slot: u8,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub dev_desc: UsbDeviceDescriptor,
    pub descs: Vec<UsbDescriptor>,
    pub ctrl_ep_ring: Box<CommandRing>,
}

impl XhciAttachInfo {
    pub fn last_config_desc(&self) -> Option<&ConfigDescriptor> {
        self.descs.iter().rev().find_map(|d| {
            if let UsbDescriptor::Config(c) = d {
                Some(c)
            } else {
                None
            }
        })
    }

    pub fn interface_descs(&self) -> Vec<&InterfaceDescriptor> {
        self.descs
            .iter()
            .filter_map(|d| {
                if let UsbDescriptor::Interface(i) = d {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn endpoint_descs(&self) -> Vec<&EndpointDescriptor> {
        self.descs
            .iter()
            .filter_map(|d| {
                if let UsbDescriptor::Endpoint(e) = d {
                    Some(e)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn ctrl_ep_ring_mut(&mut self) -> &mut CommandRing {
        &mut self.ctrl_ep_ring
    }
}

pub enum UsbDeviceAttachInfo {
    Xhci(XhciAttachInfo),
}

impl UsbDeviceAttachInfo {
    pub fn new_xhci(info: XhciAttachInfo) -> Self {
        Self::Xhci(info)
    }

    pub fn interface_name(&self) -> &'static str {
        match self {
            Self::Xhci(_) => "xhci",
        }
    }

    pub fn port(&self) -> usize {
        match self {
            Self::Xhci(info) => info.port,
        }
    }

    pub fn slot(&self) -> usize {
        match self {
            Self::Xhci(info) => info.slot as usize,
        }
    }

    pub fn vendor(&self) -> Option<&str> {
        match self {
            Self::Xhci(info) => info.vendor.as_deref(),
        }
    }

    pub fn product(&self) -> Option<&str> {
        match self {
            Self::Xhci(info) => info.product.as_deref(),
        }
    }

    pub fn serial(&self) -> Option<&str> {
        match self {
            Self::Xhci(info) => info.serial.as_deref(),
        }
    }

    pub fn interface_descs(&self) -> Vec<&InterfaceDescriptor> {
        match self {
            Self::Xhci(info) => info.interface_descs(),
        }
    }
}

pub struct UsbDevice {
    attach_info: Mutex<UsbDeviceAttachInfo>,
    hc: Arc<dyn UsbHostController>,
}

impl UsbDevice {
    pub fn new(attach_info: UsbDeviceAttachInfo, hc: Arc<dyn UsbHostController>) -> Self {
        Self {
            attach_info: Mutex::new(attach_info),
            hc,
        }
    }

    pub fn hc(&self) -> &dyn UsbHostController {
        self.hc.as_ref()
    }

    pub fn lock_attach_info(&self) -> Result<MutexGuard<'_, UsbDeviceAttachInfo>> {
        self.attach_info.try_lock()
    }

    pub fn has_interface(&self, triple: (u8, u8, u8)) -> Result<bool> {
        let attach_info = self.attach_info.try_lock()?;
        Ok(attach_info
            .interface_descs()
            .iter()
            .any(|d| d.triple() == triple))
    }

    fn describe_inner(&self) -> Result<String> {
        let info = self.attach_info.try_lock()?;

        Ok(format!(
            "({}) p{}:s{} {} - {} - {}\n",
            info.interface_name(),
            info.port(),
            info.slot(),
            info.vendor().unwrap_or("<UNKNOWN VENDOR>"),
            info.product().unwrap_or("<UNKNOWN PRODUCT>"),
            info.serial().unwrap_or("<UNKNOWN SERIAL>"),
        ))
    }
}

impl Device for UsbDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new("usb-device"))
    }

    fn describe(&self) -> Result<String> {
        self.describe_inner()
    }
}

struct UsbBus {
    usb_devices: Vec<Arc<UsbDevice>>,
}

impl UsbBus {
    const fn new() -> Self {
        Self {
            usb_devices: Vec::new(),
        }
    }
}

impl UsbBus {
    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self) -> Result<()> {
        vfs::add_dev(Arc::new(UsbBusDevice))?;
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

        for d in &self.usb_devices {
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

pub fn device_info() -> Result<DeviceInfo> {
    Ok(DeviceInfo::new(NAME))
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = USB_BUS.try_lock()?;
    driver.probe()?;
    driver.attach()?;
    kinfo!("{}: Attached!", NAME);
    Ok(())
}

pub fn register_driver(driver: Arc<dyn UsbDriver>) -> Result<()> {
    USB_DRIVERS.try_lock()?.push(driver);

    Ok(())
}

pub fn attach_usb_device(device: Arc<UsbDevice>) -> Result<()> {
    USB_BUS.try_lock()?.usb_devices.push(device.clone());

    let drivers: Vec<Arc<dyn UsbDriver>> = USB_DRIVERS.try_lock()?.clone();

    for driver in &drivers {
        match driver.probe(&device) {
            Ok(false) => continue,
            Ok(true) => {
                kinfo!("{}: {} attached", NAME, driver.name());
                return Ok(());
            }
            Err(err) => {
                kerror!("{}: {}: Failed to probe: {:?}", NAME, driver.name(), err);
                return Ok(());
            }
        }
    }

    kinfo!("{}: Unsupported USB device detected, no attached", NAME);

    Ok(())
}

struct UsbBusDevice;

impl Device for UsbBusDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl CharDevice for UsbBusDevice {
    fn read(&self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        USB_BUS.try_lock()?.read(offset, max_len)
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        USB_BUS.try_lock()?.write(data)
    }

    fn open(&self) -> Result<()> {
        USB_BUS.try_lock()?.open()
    }

    fn close(&self) -> Result<()> {
        USB_BUS.try_lock()?.close()
    }
}
