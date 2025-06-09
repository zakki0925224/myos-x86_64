use super::xhc::{self, context::endpoint::EndpointType};
use crate::{
    arch,
    device::{DeviceDriverFunction, DeviceDriverInfo},
    error::{Error, Result},
    fs::vfs,
    info,
    util::mutex::Mutex,
    warn,
};
use alloc::{boxed::Box, vec::Vec};
use descriptor::*;
use device::UsbDevice;

mod descriptor;
pub mod device;

static mut USB_BUS_DRIVER: Mutex<UsbBusDriver> = Mutex::new(UsbBusDriver::new());

#[derive(Debug, Clone, PartialEq)]
pub enum UsbBusDriverError {
    UsbDeviceError { slot_id: usize, err: Box<Error> },
    UsbDeviceNotExitstError,
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

    fn find_device_by_slot_id(&self, slot_id: usize) -> Option<UsbDevice> {
        self.usb_devices
            .iter()
            .find(|d| d.slot_id() == slot_id)
            .cloned()
    }

    fn update_device(&mut self, device: UsbDevice) -> Result<()> {
        let device_mut = self
            .usb_devices
            .iter_mut()
            .find(|d| d.slot_id() == device.slot_id())
            .ok_or(UsbBusDriverError::UsbDeviceNotExitstError)?;

        *device_mut = device;
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
        // XHC driver mut be started
        let port_ids = arch::disabled_int(|| xhc::scan_ports())?;

        for port_id in port_ids {
            arch::disabled_int(|| xhc::reset_port(port_id))?;
            let device = arch::disabled_int(|| xhc::alloc_address_to_device(port_id))?;
            self.usb_devices.push(device);
        }

        for device in self.usb_devices.iter_mut() {
            let slot_id = device.slot_id();
            arch::disabled_int(|| device.init()).map_err(|err| {
                UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                }
            })?;

            device.read_dev_desc();

            arch::disabled_int(|| device.request_to_get_desc(DescriptorType::Configuration, 0))
                .map_err(|err| UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                })?;

            device.read_conf_descs();

            let boot_interface = match device
                .get_interface_descs()
                .iter()
                .find(|d| d.class == 3 && d.sub_class == 1 && d.protocol == 1)
            {
                Some(d) => **d,
                None => {
                    warn!(
                        "usb: Unsupported device, skip configuring... (slot id: {})",
                        slot_id
                    );
                    continue;
                }
            };

            let conf_desc = match device.get_conf_descs()[0].clone() {
                Descriptor::Configuration(desc) => desc,
                _ => unreachable!(),
            };

            arch::disabled_int(|| device.configure_endpoint(EndpointType::InterruptIn)).map_err(
                |err| UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                },
            )?;

            arch::disabled_int(|| device.request_to_set_conf(conf_desc.conf_value)).map_err(
                |err| UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                },
            )?;

            arch::disabled_int(|| device.request_to_set_interface(boot_interface)).map_err(
                |err| UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                },
            )?;

            arch::disabled_int(|| device.request_to_set_protocol(boot_interface, 0)).map_err(
                |err| UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                },
            )?;

            arch::disabled_int(|| device.configure_endpoint_transfer_ring()).map_err(|err| {
                UsbBusDriverError::UsbDeviceError {
                    slot_id,
                    err: Box::new(err),
                }
            })?;

            device.is_configured = true;
        }

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

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    unsafe { USB_BUS_DRIVER.try_lock() }?.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    let driver_name = driver.get_device_driver_info()?.name;

    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver_name);

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

pub fn find_device_by_slot_id(slot_id: usize) -> Result<Option<UsbDevice>> {
    let driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    Ok(driver.find_device_by_slot_id(slot_id))
}

pub fn update_device(device: UsbDevice) -> Result<()> {
    let mut driver = unsafe { USB_BUS_DRIVER.try_lock() }?;
    driver.update_device(device)
}
