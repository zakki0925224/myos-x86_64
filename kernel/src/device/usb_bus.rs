use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    device::{
        self,
        xhc::{
            desc::*,
            register::{CommandRing, UsbHidProtocol},
        },
    },
    error::{Error, Result},
    fs::vfs,
    info, trace,
    util::mutex::Mutex,
};
use alloc::{boxed::Box, collections::btree_set::BTreeSet, string::String, vec::Vec};

static mut USB_BUS_DRIVER: Mutex<UsbBusDriver> = Mutex::new(UsbBusDriver::new());

pub struct XhciAttachInfo {
    port: usize,
    slot: u8,
    vendor: Option<String>,
    product: Option<String>,
    serial: Option<String>,
    dev_desc: UsbDeviceDescriptor,
    descs: Vec<UsbDescriptor>,
    ctrl_ep_ring: Box<CommandRing>,
}

impl XhciAttachInfo {
    fn last_config_desc(&self) -> Option<&ConfigDescriptor> {
        self.descs.iter().rev().find_map(|d| {
            if let UsbDescriptor::Config(c) = d {
                Some(c)
            } else {
                None
            }
        })
    }

    fn interface_descs(&self) -> Vec<&InterfaceDescriptor> {
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

    fn endpoint_descs(&self) -> Vec<&EndpointDescriptor> {
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

    fn ctrl_ep_ring_mut(&mut self) -> &mut CommandRing {
        &mut self.ctrl_ep_ring
    }
}

pub enum UsbDeviceAttachInfo {
    Xhci(XhciAttachInfo),
}

impl UsbDeviceAttachInfo {
    pub fn new_xhci(
        port: usize,
        slot: u8,
        vendor: Option<String>,
        product: Option<String>,
        serial: Option<String>,
        dev_desc: UsbDeviceDescriptor,
        descs: Vec<UsbDescriptor>,
        ctrl_ep_ring: Box<CommandRing>,
    ) -> Self {
        Self::Xhci(XhciAttachInfo {
            port,
            slot,
            vendor,
            product,
            serial,
            dev_desc,
            descs,
            ctrl_ep_ring,
        })
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
}

impl UsbDevice {
    pub fn new(attach_info: UsbDeviceAttachInfo) -> Self {
        Self {
            attach_info,
            state: UsbDeviceState::Attached,
        }
    }

    fn configure_xhci_keyboard(&mut self) -> Result<()> {
        let xhci_info = match &mut self.attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;

        // set config
        let config_desc = xhci_info
            .last_config_desc()
            .ok_or(Error::Failed("No configuration descriptor found"))?;
        let config_value = config_desc.config_value();
        device::xhc::request(|xhc| {
            xhc.set_config(slot, xhci_info.ctrl_ep_ring_mut(), config_value)
        })?;

        // set interface
        let interface_descs = xhci_info.interface_descs();
        let target_interface_desc = *interface_descs
            .iter()
            .find(|d| d.triple() == (3, 1, 1))
            .ok_or(Error::Failed("No target interface descriptor found"))?;
        let interface_num = target_interface_desc.interface_num;
        let alt_setting = target_interface_desc.alt_setting;
        device::xhc::request(|xhc| {
            xhc.set_interface(
                slot,
                xhci_info.ctrl_ep_ring_mut(),
                interface_num,
                alt_setting,
            )
        })?;

        // set protocol
        let protocol = UsbHidProtocol::BootProtocol as u8;
        device::xhc::request(|xhc| {
            xhc.set_protocol(slot, xhci_info.ctrl_ep_ring_mut(), interface_num, protocol)
        })?;

        self.state = UsbDeviceState::Configured;
        trace!("USB device configured: slot {}", slot);
        Ok(())
    }

    fn poll_xhci_keyboard(&mut self) -> Result<()> {
        let xhci_info = match &mut self.attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;

        let mut prev_pressed = BTreeSet::new();
        loop {
            let pressed = {
                let report =
                    device::xhc::request(|xhc| xhc.hid_report(slot, xhci_info.ctrl_ep_ring_mut()))?;
                BTreeSet::from_iter(report.into_iter().skip(2).filter(|id| *id != 0))
            };
            let diff = pressed.symmetric_difference(&prev_pressed);
            for id in diff {
                if pressed.contains(id) {
                    info!("USB keyboard key pressed: {}", id);
                } else {
                    info!("USB keyboard key released: {}", id);
                }
            }
            prev_pressed = pressed;
        }

        Ok(())
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
                    dev.configure_xhci_keyboard()?;
                }
                UsbDeviceState::Configured => {
                    dev.poll_xhci_keyboard()?;
                }
            }
        }

        Ok(())
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
    let driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.get_device_driver_info()?.name);
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
    info!(
        "{}: New USB device attached!",
        driver.get_device_driver_info()?.name,
    );
    Ok(())
}

pub fn poll_normal() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.poll_normal()
}
