use crate::{
    device::{
        usb::{
            xhc::{desc::*, register::*},
            UsbDeviceDriverFunction,
        },
        DeviceDriverFunction, DeviceDriverInfo,
    },
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
};
use alloc::{boxed::Box, string::String, vec::Vec};

static mut USB_BUS_DRIVER: Mutex<UsbBusDriver> = Mutex::new(UsbBusDriver::new());

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
            Self::Xhci(info) => info.vendor.as_ref().map(|s| s.as_str()),
        }
    }

    pub fn product(&self) -> Option<&str> {
        match self {
            Self::Xhci(info) => info.product.as_ref().map(|s| s.as_str()),
        }
    }

    pub fn serial(&self) -> Option<&str> {
        match self {
            Self::Xhci(info) => info.serial.as_ref().map(|s| s.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbDeviceState {
    Attached, // addressed by host controller
    Configured,
}

pub struct UsbDevice {
    attach_info: UsbDeviceAttachInfo,
    state: UsbDeviceState,
    driver: Box<dyn UsbDeviceDriverFunction>,
}

impl UsbDevice {
    pub fn new(attach_info: UsbDeviceAttachInfo, driver: Box<dyn UsbDeviceDriverFunction>) -> Self {
        Self {
            attach_info,
            state: UsbDeviceState::Attached,
            driver,
        }
    }
}

struct UsbBusDriver {
    device_driver_info: DeviceDriverInfo,
    usb_devices: Vec<UsbDevice>,
}

impl UsbBusDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("usb-bus"),
            usb_devices: Vec::new(),
        }
    }

    fn attach_usb_device(&mut self, device: UsbDevice) -> Result<()> {
        self.usb_devices.push(device);
        Ok(())
    }
}

impl DeviceDriverFunction for UsbBusDriver {
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

        for d in &self.usb_devices {
            let info = &d.attach_info;
            let interface = info.interface_name();
            let port = info.port();
            let slot = info.slot();
            let vendor = info.vendor().unwrap_or("<UNKNOWN VENDOR>");
            let serial = info.serial().unwrap_or("<UNKNOWN SERIAL>");
            let product = info.product().unwrap_or("<UNKNOWN PRODUCT>");

            s.push_str(&format!(
                "({}) p{}:s{} {} - {} - {}\n",
                interface, port, slot, vendor, product, serial
            ));
        }

        Ok(s.into_bytes())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);
    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn attach_usb_device(device: UsbDevice) -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.attach_usb_device(device)?;
    Ok(())
}

pub fn poll_normal() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.poll_normal()
}
