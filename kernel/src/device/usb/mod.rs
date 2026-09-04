use crate::{
    device::usb::{usb_bus::UsbDevice, xhc::register::CommandRing},
    error::Result,
};
use alloc::{sync::Arc, vec::Vec};

pub mod hid_keyboard;
pub mod hid_tablet;
pub mod usb_bus;
pub mod xhc;

pub trait UsbHostController: Send + Sync {
    fn set_config(&self, slot: u8, ctrl_ep_ring: &mut CommandRing, config_value: u8) -> Result<()>;
    fn set_interface(
        &self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        alt_setting: u8,
    ) -> Result<()>;
    fn set_protocol(
        &self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        protocol: u8,
    ) -> Result<()>;
    fn hid_report(&self, slot: u8, ctrl_ep_ring: &mut CommandRing) -> Result<Vec<u8>>;
    fn hid_report_desc(
        &self,
        slot: u8,
        ctrl_ep_ring: &mut CommandRing,
        interface_num: u8,
        desc_size: usize,
    ) -> Result<Vec<u8>>;
}

pub trait UsbDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self, dev: &Arc<UsbDevice>) -> Result<bool>;
}
