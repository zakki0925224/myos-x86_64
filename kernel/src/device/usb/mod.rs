use crate::{device::usb::usb_bus::UsbDeviceAttachInfo, error::Result};

pub mod hid_keyboard;
pub mod hid_tablet;
pub mod usb_bus;
pub mod xhc;

pub trait UsbDeviceDriverFunction {
    fn configure(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()>;
    fn poll(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()>;
}
