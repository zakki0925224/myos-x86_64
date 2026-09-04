use crate::{
    device::{
        keyboard, register_pollable,
        usb::{usb_bus::*, xhc::register::*, UsbDriver, UsbHostController},
        Device, DeviceInfo, Pollable,
    },
    error::{Error, Result},
    sync::mutex::Mutex,
    util::{
        self,
        keyboard::{key_event::*, key_map::*, scan_code::*},
    },
};
use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::Arc,
};

const NAME: &str = "usb-hid-keyboard";
const INTERFACE_TRIPLE: (u8, u8, u8) = (3, 1, 1);

struct Inner {
    configured: bool,
    key_map: BTreeMap<u8, ScanCode>,
    mod_keys_state: ModifierKeysState,
    prev_pressed: BTreeSet<u8>,
}

pub struct UsbHidKeyboardDevice {
    dev: Arc<UsbDevice>,
    inner: Mutex<Inner>,
}

impl Inner {
    fn configure(
        &mut self,
        hc: &dyn UsbHostController,
        attach_info: &mut UsbDeviceAttachInfo,
    ) -> Result<()> {
        let UsbDeviceAttachInfo::Xhci(xhci_info) = attach_info;
        let slot = xhci_info.slot;

        // set config
        let config_desc = xhci_info
            .last_config_desc()
            .ok_or(Error::NotFound.with_context("Configuration descriptor"))?;
        let config_value = config_desc.config_value();
        hc.set_config(slot, xhci_info.ctrl_ep_ring_mut(), config_value)?;

        // set interface
        let interface_descs = xhci_info.interface_descs();
        let target_interface_desc = *interface_descs
            .iter()
            .find(|d| d.triple() == (3, 1, 1))
            .ok_or(Error::NotFound.with_context("Interface descriptor"))?;
        let interface_num = target_interface_desc.interface_num;
        let alt_setting = target_interface_desc.alt_setting;
        hc.set_interface(
            slot,
            xhci_info.ctrl_ep_ring_mut(),
            interface_num,
            alt_setting,
        )?;

        // set protocol
        let protocol = UsbHidProtocol::BootProtocol as u8;
        hc.set_protocol(slot, xhci_info.ctrl_ep_ring_mut(), interface_num, protocol)?;

        Ok(())
    }

    fn poll(
        &mut self,
        hc: &dyn UsbHostController,
        attach_info: &mut UsbDeviceAttachInfo,
    ) -> Result<()> {
        let UsbDeviceAttachInfo::Xhci(xhci_info) = attach_info;
        let slot = xhci_info.slot;

        let report = hc.hid_report(slot, xhci_info.ctrl_ep_ring_mut())?;

        let modifier = report.first().copied().unwrap_or(0);
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

            let e = util::keyboard::key_event_from_usb_hid(
                &self.key_map,
                &self.mod_keys_state,
                key_state,
                *id,
            );

            if let Some(e) = e {
                keyboard::push_key_event(e)?;
            }
        }

        self.prev_pressed = pressed;

        Ok(())
    }
}

impl UsbHidKeyboardDevice {
    fn new(dev: Arc<UsbDevice>, key_map: KeyMap) -> Self {
        Self {
            dev,
            inner: Mutex::new(Inner {
                configured: false,
                prev_pressed: BTreeSet::new(),
                key_map: key_map.to_usb_hid_map(),
                mod_keys_state: ModifierKeysState::default(),
            }),
        }
    }
}

impl Device for UsbHidKeyboardDevice {
    fn info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo::new(NAME))
    }
}

impl Pollable for UsbHidKeyboardDevice {
    fn poll(&self) -> Result<()> {
        let mut inner = self.inner.try_lock()?;
        let mut attach_info = self.dev.lock_attach_info()?;
        let hc = self.dev.hc();

        if !inner.configured {
            inner.configure(hc, &mut attach_info)?;
            inner.configured = true;
            return Ok(());
        }

        inner.poll(hc, &mut attach_info)
    }
}

pub struct UsbHidKeyboardDriver;

impl UsbDriver for UsbHidKeyboardDriver {
    fn name(&self) -> &'static str {
        NAME
    }

    fn probe(&self, dev: &Arc<UsbDevice>) -> Result<bool> {
        if !dev.has_interface(INTERFACE_TRIPLE)? {
            return Ok(false);
        }

        register_pollable(Arc::new(UsbHidKeyboardDevice::new(
            dev.clone(),
            JIS_JP_109_KEY_MAP,
        )))?;

        Ok(true)
    }
}
