use crate::{
    device::{
        usb::{
            hid_keyboard, hid_tablet,
            xhc::{desc::*, register::CommandRing},
            UsbDriver,
        },
        DeviceInfo, Driver,
    },
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
};
use alloc::{boxed::Box, string::String, vec::Vec};

const NAME: &str = "usb-bus";

static USB_BUS_DRIVER: Mutex<UsbBusDriver> = Mutex::new(UsbBusDriver::new());

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

enum UsbDeviceState {
    Attached,
    Configured,
}

pub struct UsbDevice {
    attach_info: UsbDeviceAttachInfo,
    state: UsbDeviceState,
    driver: Box<dyn UsbDriver>,
}

impl UsbDevice {
    fn describe(&self) -> String {
        format!(
            "({}) p{}:s{} {} - {} - {}\n",
            self.attach_info.interface_name(),
            self.attach_info.port(),
            self.attach_info.slot(),
            self.attach_info.vendor().unwrap_or("<UNKNOWN VENDOR>"),
            self.attach_info.product().unwrap_or("<UNKNOWN PRODUCT>"),
            self.attach_info.serial().unwrap_or("<UNKNOWN SERIAL>"),
        )
    }
}

const USB_DRIVER_PROBES: &[fn(&UsbDeviceAttachInfo) -> Option<Box<dyn UsbDriver>>] =
    &[hid_keyboard::probe, hid_tablet::probe];

struct UsbBusDriver {
    usb_devices: Vec<UsbDevice>,
}

impl UsbBusDriver {
    const fn new() -> Self {
        Self {
            usb_devices: Vec::new(),
        }
    }
}

impl Driver for UsbBusDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> Result<()> {
        for dev in &mut self.usb_devices {
            match dev.state {
                // configure attached devices
                UsbDeviceState::Attached => {
                    dev.driver.configure(&mut dev.attach_info)?;
                    dev.state = UsbDeviceState::Configured;
                }
                UsbDeviceState::Configured => {
                    dev.driver.poll(&mut dev.attach_info)?;
                }
            }
        }

        Ok(())
    }

    fn read(&mut self, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let mut s = String::new();

        for d in &self.usb_devices {
            s.push_str(&d.describe());
        }

        let bytes = s.into_bytes();
        let start = offset.min(bytes.len());
        let end = start.saturating_add(max_len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

pub fn probe_and_attach() -> Result<()> {
    USB_BUS_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&USB_BUS_DRIVER)?;
    kinfo!("{}: Attached!", NAME);
    Ok(())
}

pub fn poll_normal() -> Result<()> {
    USB_BUS_DRIVER.try_lock()?.poll()
}

pub fn attach_usb_device(attach_info: UsbDeviceAttachInfo) -> Result<()> {
    for probe in USB_DRIVER_PROBES {
        if let Some(driver) = probe(&attach_info) {
            kinfo!("{}: {} attached", NAME, driver.name());
            USB_BUS_DRIVER.try_lock()?.usb_devices.push(UsbDevice {
                attach_info,
                state: UsbDeviceState::Attached,
                driver,
            });
            return Ok(());
        }
    }

    kinfo!("{}: Unsupported USB device detected, no attached", NAME);

    Ok(())
}
