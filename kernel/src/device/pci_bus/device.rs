use crate::{
    arch::x86_64::registers::*,
    device::pci_bus::conf_space::{self, *},
    error::{Error, Result},
};
use alloc::vec::Vec;

pub trait PciDeviceFunction {
    fn bdf(&self) -> (usize, usize, usize);
    fn read_conf_space_header(&self) -> Result<ConfigurationSpaceCommonHeaderField>;
    fn write_conf_space_header(&self, value: ConfigurationSpaceCommonHeaderField) -> Result<()>;
    fn read_conf_space_non_bridge_field(&self) -> Result<ConfigurationSpaceNonBridgeField>;
    fn read_conf_space_pci_to_pci_bridge_field(
        &self,
    ) -> Result<ConfigurationSpacePciToPciBridgeField>;
    fn read_space_pci_to_cardbus_bridge_field(&self)
        -> Result<ConfigurationSpacePciToCardBusField>;
    fn read_interrupt_line(&self) -> Result<u8>;
    fn write_interrupt_line(&self, value: u8) -> Result<()>;
    fn device_class(&self) -> (u8, u8, u8);
    fn is_available_msi_int(&self) -> bool;
    fn read_msi_caps_list(&self) -> Vec<MsiCapabilityField>;
    fn set_msi_cap(
        &self,
        msg_addr: MsiMessageAddressField,
        msg_data: MsiMessageDataField,
    ) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct PciDevice {
    bdf: (usize, usize, usize),
}

impl PciDevice {
    pub fn try_new(bus: usize, device: usize, func: usize) -> Option<Self> {
        let conf_space_header =
            ConfigurationSpaceCommonHeaderField::read(bus, device, func).ok()?;

        if !conf_space_header.is_exist() {
            return None;
        }

        Some(Self {
            bdf: (bus, device, func),
        })
    }

    fn read_caps_ptr(&self) -> Option<u8> {
        let conf_space_header = self.read_conf_space_header().ok()?;

        if !conf_space_header.status.caps_list_available() {
            return None;
        }

        match conf_space_header.get_header_type() {
            ConfigurationSpaceHeaderType::NonBridge
            | ConfigurationSpaceHeaderType::MultiFunction => {
                Some(self.read_conf_space_non_bridge_field().unwrap().caps_ptr)
            }
            ConfigurationSpaceHeaderType::PciToPciBridge => Some(
                self.read_conf_space_pci_to_pci_bridge_field()
                    .unwrap()
                    .caps_ptr,
            ),
            _ => None, // unsupported type
        }
    }
}

impl PciDeviceFunction for PciDevice {
    fn bdf(&self) -> (usize, usize, usize) {
        self.bdf
    }

    fn read_conf_space_header(&self) -> Result<ConfigurationSpaceCommonHeaderField> {
        let (bus, device, func) = self.bdf;
        ConfigurationSpaceCommonHeaderField::read(bus, device, func)
    }

    fn write_conf_space_header(&self, value: ConfigurationSpaceCommonHeaderField) -> Result<()> {
        let (bus, device, func) = self.bdf;
        value.write(bus, device, func)
    }

    fn read_conf_space_non_bridge_field(&self) -> Result<ConfigurationSpaceNonBridgeField> {
        let (bus, device, func) = self.bdf;

        match self.read_conf_space_header()?.get_header_type() {
            ConfigurationSpaceHeaderType::NonBridge
            | ConfigurationSpaceHeaderType::MultiFunction => {
                ConfigurationSpaceNonBridgeField::read(bus, device, func)
            }
            _ => Err("Invalid configuration space header type".into()),
        }
    }

    fn read_conf_space_pci_to_pci_bridge_field(
        &self,
    ) -> Result<ConfigurationSpacePciToPciBridgeField> {
        let (bus, device, func) = self.bdf;

        match self.read_conf_space_header()?.get_header_type() {
            ConfigurationSpaceHeaderType::PciToPciBridge => {
                ConfigurationSpacePciToPciBridgeField::read(bus, device, func)
            }
            _ => Err("Invalid configuration space header type".into()),
        }
    }

    fn read_space_pci_to_cardbus_bridge_field(
        &self,
    ) -> Result<ConfigurationSpacePciToCardBusField> {
        let (bus, device, func) = self.bdf;

        match self.read_conf_space_header()?.get_header_type() {
            ConfigurationSpaceHeaderType::PciToCardBusBridge => {
                ConfigurationSpacePciToCardBusField::read(bus, device, func)
            }
            _ => Err("Invalid configuration space header type".into()),
        }
    }

    fn read_interrupt_line(&self) -> Result<u8> {
        let (bus, device, func) = self.bdf;

        let data = conf_space::read_conf_space(bus, device, func, 0x3c)?;
        Ok(data as u8)
    }

    fn write_interrupt_line(&self, value: u8) -> Result<()> {
        let (bus, device, func) = self.bdf;

        let data = conf_space::read_conf_space(bus, device, func, 0x3c)? & !0xff | value as u32;
        conf_space::write_conf_space(bus, device, func, 0x3c, data)?;

        Ok(())
    }

    fn device_class(&self) -> (u8, u8, u8) {
        let conf_space_header = self.read_conf_space_header().unwrap();

        let class = conf_space_header.class_code;
        let subclass = conf_space_header.subclass;
        let prog_if = conf_space_header.prog_if;
        (class, subclass, prog_if)
    }

    fn is_available_msi_int(&self) -> bool {
        self.read_conf_space_header()
            .unwrap()
            .status
            .caps_list_available()
    }

    fn read_msi_caps_list(&self) -> Vec<MsiCapabilityField> {
        let (bus, device, func) = self.bdf;
        let mut list = Vec::new();

        if !self.is_available_msi_int() {
            return list;
        }

        if let Some(caps_ptr) = self.read_caps_ptr() {
            let mut caps_ptr = caps_ptr as usize;
            while caps_ptr != 0 {
                if let Ok(field) = MsiCapabilityField::read(bus, device, func, caps_ptr) {
                    caps_ptr = field.next_ptr as usize;
                    list.push(field);
                } else {
                    break;
                }
            }
        }

        list
    }

    fn set_msi_cap(
        &self,
        msg_addr: MsiMessageAddressField,
        msg_data: MsiMessageDataField,
    ) -> Result<()> {
        let caps_ptr = self
            .read_caps_ptr()
            .ok_or::<Error>("Failed to read MSI capability fields".into())?;

        let mut cap = MsiCapabilityField::default();
        let mut caps_ptr = caps_ptr as usize;
        let caps_list = self.read_msi_caps_list();
        let caps_list_len = caps_list.len();

        if caps_list_len == 0 {
            return Err("MSI capability fields was not found".into());
        }

        for (i, field) in caps_list.iter().enumerate() {
            if field.cap_id == 5 {
                cap = *field;
                break;
            }

            caps_ptr = field.next_ptr as usize;

            if i == caps_list_len - 1 {
                return Err("MSI capability fields was not found".into());
            }
        }

        let mut msg_ctrl = cap.msg_ctrl;
        msg_ctrl.set_is_enable(true);
        msg_ctrl.set_multiple_msg_enable(0);
        cap.msg_ctrl = msg_ctrl;
        cap.msg_addr_low = msg_addr;
        cap.msg_data = msg_data;

        // write cap
        let (bus, device, func) = self.bdf;
        cap.write(bus, device, func, caps_ptr)?;

        Ok(())
    }
}
