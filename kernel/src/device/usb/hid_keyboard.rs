use crate::{
    device::{
        self,
        usb::{usb_bus::*, xhc::register::UsbHidProtocol, UsbDeviceDriverFunction},
    },
    error::{Error, Result},
    info,
};
use alloc::collections::btree_set::BTreeSet;

pub struct UsbHidKeyboardDriver {
    prev_pressed: BTreeSet<u8>,
}

impl UsbDeviceDriverFunction for UsbHidKeyboardDriver {
    fn configure(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()> {
        let xhci_info = match attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;

        // set config
        let config_desc = xhci_info
            .last_config_desc()
            .ok_or(Error::Failed("No configuration descriptor found"))?;
        let config_value = config_desc.config_value();
        device::usb::xhc::request(|xhc| {
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
        device::usb::xhc::request(|xhc| {
            xhc.set_interface(
                slot,
                xhci_info.ctrl_ep_ring_mut(),
                interface_num,
                alt_setting,
            )
        })?;

        // set protocol
        let protocol = UsbHidProtocol::BootProtocol as u8;
        device::usb::xhc::request(|xhc| {
            xhc.set_protocol(slot, xhci_info.ctrl_ep_ring_mut(), interface_num, protocol)
        })?;

        Ok(())
    }

    fn poll(&mut self, attach_info: &mut UsbDeviceAttachInfo) -> Result<()> {
        let xhci_info = match attach_info {
            UsbDeviceAttachInfo::Xhci(info) => info,
        };
        let slot = xhci_info.slot;

        let pressed = {
            let report = device::usb::xhc::request(|xhc| {
                xhc.hid_report(slot, xhci_info.ctrl_ep_ring_mut())
            })?;
            BTreeSet::from_iter(report.into_iter().skip(2).filter(|id| *id != 0))
        };
        let diff = pressed.symmetric_difference(&self.prev_pressed);
        for id in diff {
            if pressed.contains(id) {
                info!("USB HID Keyboard: Key {} pressed", id);
            } else {
                info!("USB HID Keyboard: Key {} released", id);
            }
        }
        self.prev_pressed = pressed;

        Ok(())
    }
}

impl UsbHidKeyboardDriver {
    pub fn new() -> Self {
        Self {
            prev_pressed: BTreeSet::new(),
        }
    }
}
