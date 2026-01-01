use crate::{
    device::{
        self, tty,
        usb::{usb_bus::*, xhc::register::*, UsbDeviceDriverFunction},
    },
    error::{Error, Result},
    util::{
        self,
        keyboard::{key_event::*, key_map::*, scan_code::*},
    },
};
use alloc::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

pub struct UsbHidKeyboardDriver {
    pub name: &'static str,
    key_map: BTreeMap<u8, ScanCode>,
    mod_keys_state: ModifierKeysState,
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
            .ok_or::<Error>("No configuration descriptor found".into())?;
        let config_value = config_desc.config_value();
        device::usb::xhc::request(|xhc| {
            xhc.set_config(slot, xhci_info.ctrl_ep_ring_mut(), config_value)
        })?;

        // set interface
        let interface_descs = xhci_info.interface_descs();
        let target_interface_desc =
            *interface_descs
                .iter()
                .find(|d| d.triple() == (3, 1, 1))
                .ok_or::<Error>("No target interface descriptor found".into())?;
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

        let report =
            device::usb::xhc::request(|xhc| xhc.hid_report(slot, xhci_info.ctrl_ep_ring_mut()))?;

        let modifier = report.get(0).copied().unwrap_or(0);
        let ctrl = (modifier & 0x01 != 0) || (modifier & 0x10 != 0);
        let shift = (modifier & 0x02 != 0) || (modifier & 0x20 != 0);
        let alt = (modifier & 0x04 != 0) | (modifier & 0x40 != 0);
        let gui = (modifier & 0x08 != 0) || (modifier & 0x80 != 0);

        self.mod_keys_state.ctrl = ctrl;
        self.mod_keys_state.shift = shift;
        self.mod_keys_state.alt = alt;
        self.mod_keys_state.gui = gui;

        let pressed = BTreeSet::from_iter(report.into_iter().skip(2).filter(|id| *id != 0));
        let diff = pressed.symmetric_difference(&self.prev_pressed);

        for id in diff {
            let key_state = if pressed.contains(id) {
                KeyState::Pressed
            } else {
                KeyState::Released
            };

            let e = util::keyboard::get_key_event_from_usb_hid(
                &self.key_map,
                &self.mod_keys_state,
                key_state,
                *id,
            );

            if let Some(e) = e {
                if e.state == KeyState::Pressed {
                    match e.code {
                        KeyCode::CursorUp => {
                            tty::input('\x1b')?;
                            tty::input('[')?;
                            tty::input('A')?;
                        }
                        KeyCode::CursorDown => {
                            tty::input('\x1b')?;
                            tty::input('[')?;
                            tty::input('B')?;
                        }
                        KeyCode::CursorRight => {
                            tty::input('\x1b')?;
                            tty::input('[')?;
                            tty::input('C')?;
                        }
                        KeyCode::CursorLeft => {
                            tty::input('\x1b')?;
                            tty::input('[')?;
                            tty::input('D')?;
                        }
                        _ => {
                            if let Some(c) = e.c {
                                tty::input(c)?;
                            }
                        }
                    }
                }
            }
        }
        self.prev_pressed = pressed;

        Ok(())
    }
}

impl UsbHidKeyboardDriver {
    pub fn new(key_map: KeyMap) -> Self {
        Self {
            name: "usb-hid-keyboard",
            prev_pressed: BTreeSet::new(),
            key_map: key_map.to_usb_hid_map(),
            mod_keys_state: ModifierKeysState::default(),
        }
    }
}
