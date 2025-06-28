use crate::{
    device::{
        self,
        usb::{usb_bus::*, xhc::register::UsbHidProtocol, UsbDeviceDriverFunction},
    },
    error::{Error, Result},
};
use alloc::collections::btree_set::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
enum KeyEvent {
    None,
    Char(char),
    Unknown(u8),
    Enter,
}

impl KeyEvent {
    fn from_usb_key_id(usage_id: u8) -> Self {
        match usage_id {
            0x00 => Self::None,
            0x04..=0x1d => Self::Char((b'a' + usage_id - 4) as char), // a-z
            0x1e..=0x27 => Self::Char((b'1' + (usage_id - 0x1e)) as char), // 1-9,0
            0x28 => Self::Enter,
            0x29 => Self::Char(0x1b as char), // ESC
            0x2a => Self::Char(0x08 as char), // Backspace
            0x2b => Self::Char('\t'),         // Tab
            0x2c => Self::Char(' '),          // Space
            0x2d => Self::Char('-'),
            0x2e => Self::Char('='),
            0x2f => Self::Char('['),
            0x30 => Self::Char(']'),
            0x31 => Self::Char('\\'),
            0x32 => Self::Char('#'),
            0x33 => Self::Char(';'),
            0x34 => Self::Char('\''),
            0x35 => Self::Char('`'),
            0x36 => Self::Char(','),
            0x37 => Self::Char('.'),
            0x38 => Self::Char('/'),
            // 0x39: CapsLock, 0x3a-0x45: F1-F12
            _ => Self::Unknown(usage_id),
        }
    }

    fn to_char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            Self::Enter => Some('\n'),
            _ => None,
        }
    }
}

pub struct UsbHidKeyboardDriver {
    pub name: &'static str,
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
            let e = KeyEvent::from_usb_key_id(*id);
            if pressed.contains(id) {
                if let Some(c) = e.to_char() {
                    device::tty::input(c)?;
                }
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
            name: "usb-hid-keyboard",
        }
    }
}
