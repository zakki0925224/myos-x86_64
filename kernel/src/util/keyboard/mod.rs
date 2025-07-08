use crate::util::keyboard::{key_event::*, scan_code::ScanCode};
use alloc::collections::btree_map::BTreeMap;

pub mod key_event;
pub mod key_map;
pub mod scan_code;

pub fn get_key_event_from_ps2(
    key_map: &BTreeMap<[u8; 6], ScanCode>,
    mod_keys_state: &mut ModifierKeysState,
    code: [u8; 6],
) -> Option<KeyEvent> {
    let scan_code = key_map.get(&code)?;

    let key_code = scan_code.key_code;
    let key_state = match scan_code {
        sc if sc.ps2_scan_code_pressed == code => KeyState::Pressed,
        sc if sc.ps2_scan_code_released == code => KeyState::Released,
        _ => unreachable!(),
    };

    // update state
    if key_code.is_shift() {
        mod_keys_state.shift = key_state == KeyState::Pressed;
    } else if key_code.is_ctrl() {
        mod_keys_state.ctrl = key_state == KeyState::Pressed;
    } else if key_code.is_gui() {
        mod_keys_state.gui = key_state == KeyState::Pressed;
    } else if key_code.is_alt() {
        mod_keys_state.alt = key_state == KeyState::Pressed;
    }

    if key_state == KeyState::Released {
        return None;
    }

    let mut c = if mod_keys_state.shift {
        scan_code.on_shift_c
    } else {
        scan_code.c
    };

    if c.is_some() && mod_keys_state.ctrl {
        match c.unwrap() as u8 {
            0x40..=0x5f => {
                c = Some((c.unwrap() as u8 - 0x40) as char);
            }
            0x60..=0x7f => {
                c = Some((c.unwrap() as u8 - 0x60) as char);
            }
            _ => (),
        }
    }

    let key_event = KeyEvent {
        code: key_code,
        state: key_state,
        c,
    };
    Some(key_event)
}

pub fn get_key_event_from_usb_hid(
    key_map: &BTreeMap<u8, ScanCode>,
    mod_keys_state: &ModifierKeysState,
    key_state: KeyState,
    usage_id: u8,
) -> Option<KeyEvent> {
    let scan_code = key_map.get(&usage_id)?;

    let key_code = scan_code.key_code;
    assert!(usage_id == scan_code.usb_hid_usage_id);

    if key_state == KeyState::Released {
        return None;
    }

    let mut c = if mod_keys_state.shift {
        scan_code.on_shift_c
    } else {
        scan_code.c
    };

    if c.is_some() && mod_keys_state.ctrl {
        match c.unwrap() as u8 {
            0x40..=0x5f => {
                c = Some((c.unwrap() as u8 - 0x40) as char);
            }
            0x60..=0x7f => {
                c = Some((c.unwrap() as u8 - 0x60) as char);
            }
            _ => (),
        }
    }

    let key_event = KeyEvent {
        code: key_code,
        state: key_state,
        c,
    };
    Some(key_event)
}
