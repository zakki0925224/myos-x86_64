use crate::{
    arch::{x86_64::registers::*, IoPortAddress, PhysicalAddress},
    error::{Error, Result},
    mem::paging::{self, *},
};
use alloc::vec::Vec;
use core::mem::transmute;
use pci_ids::*;

const PCI_PORT_CONF_REG_ADDR: IoPortAddress = IoPortAddress::new(0xcf8);
const PCI_PORT_CONF_DATA_REG_ADDR: IoPortAddress = IoPortAddress::new(0xcfc);
const PCI_DEVICE_NON_EXIST: u16 = 0xffff;
pub const PCI_DEVICE_BUS_LEN: usize = 256;
pub const PCI_DEVICE_DEVICE_LEN: usize = 32;
pub const PCI_DEVICE_FUNC_LEN: usize = 8;
const PCI_CONF_UNIQUE_FIELD_OFFSET: usize = 16;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ConfigurationSpaceCommandRegister(u16);

impl ConfigurationSpaceCommandRegister {
    pub fn write_bus_master_enable(&mut self, value: bool) {
        self.0 = (self.0 & !0x4) | ((value as u16) << 2);
    }

    pub fn write_int_disable(&mut self, value: bool) {
        self.0 = (self.0 & !0x400) | ((value as u16) << 10);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ConfigurationSpaceStatusRegister(u16);

impl ConfigurationSpaceStatusRegister {
    pub fn caps_list_available(&self) -> bool {
        (self.0 & 0x10) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigurationSpaceHeaderType {
    NonBridge,
    PciToPciBridge,
    PciToCardBusBridge,
    MultiFunction,
    Invalid(u8),
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ConfigurationSpaceCommonHeaderField {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: ConfigurationSpaceCommandRegister,
    pub status: ConfigurationSpaceStatusRegister,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
}

impl ConfigurationSpaceCommonHeaderField {
    pub fn read(bus: usize, device: usize, func: usize) -> Result<Self> {
        let mut data: [u32; 4] = [0; 4];
        for (i, elem) in data.iter_mut().enumerate() {
            let d = read_conf_space(bus, device, func, i * 4)?;
            *elem = d;
        }

        Ok(unsafe { transmute::<[u32; 4], Self>(data) })
    }

    pub fn write(&self, bus: usize, device: usize, func: usize) -> Result<()> {
        let data = unsafe { transmute::<Self, [u32; 4]>(*self) };
        for (i, elem) in data.iter().enumerate() {
            write_conf_space(bus, device, func, i * 4, *elem)?;
        }

        Ok(())
    }

    pub fn is_exist(&self) -> bool {
        self.vendor_id != PCI_DEVICE_NON_EXIST
    }

    pub fn get_device_name(&self) -> Option<&str> {
        let vendor = self.get_vendor();
        if !self.is_exist() || vendor.is_none() {
            return None;
        }

        let device = self.get_device(&vendor.unwrap());

        if device.is_some() {
            Some(device.unwrap().name())
        } else {
            None
        }
    }

    pub fn get_vendor_name(&self) -> Option<&str> {
        if !self.is_exist() {
            return None;
        }

        let vendor = self.get_vendor();

        if vendor.is_some() {
            Some(vendor.unwrap().name())
        } else {
            None
        }
    }

    pub fn get_class_name(&self) -> Option<&str> {
        if !self.is_exist() {
            return None;
        }

        let class = self.get_class();

        if class.is_some() {
            Some(class.unwrap().name())
        } else {
            None
        }
    }

    pub fn get_subclass_name(&self) -> Option<&str> {
        let subclass = self.get_subclass();
        if !self.is_exist() || subclass.is_none() {
            return None;
        }

        Some(subclass.unwrap().name())
    }

    pub fn get_header_type(&self) -> ConfigurationSpaceHeaderType {
        match self.header_type {
            0x00 => ConfigurationSpaceHeaderType::NonBridge,
            0x01 => ConfigurationSpaceHeaderType::PciToPciBridge,
            0x02 => ConfigurationSpaceHeaderType::PciToCardBusBridge,
            other => {
                if other & 0x80 != 0 {
                    ConfigurationSpaceHeaderType::MultiFunction
                } else {
                    ConfigurationSpaceHeaderType::Invalid(other)
                }
            }
        }
    }

    fn get_vendor(&self) -> Option<&Vendor> {
        Vendors::iter().find(|v| v.id() == self.vendor_id)
    }

    fn get_device(&self, vendor: &Vendor) -> Option<&Device> {
        vendor.devices().find(|d| d.id() == self.device_id)
    }

    fn get_class(&self) -> Option<&Class> {
        Classes::iter().find(|c| c.id() == self.class_code)
    }

    fn get_subclass(&self) -> Option<&Subclass> {
        match self.get_class() {
            Some(class) => class.subclasses().find(|c| c.id() == self.subclass),
            None => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseAddress {
    MemoryAddress32BitSpace(PhysicalAddress, bool), // (addr, is prefetchable)
    MemoryAddress64BitSpace(PhysicalAddress, bool),
    MmioAddressSpace(u32),
}
#[derive(Debug, Clone, Copy)]
pub struct BaseAddressRegister(u32);

impl BaseAddressRegister {
    pub fn read(&self) -> u32 {
        self.0
    }

    pub fn get_base_addr(&self) -> Option<BaseAddress> {
        let bar = self.read();

        if bar == 0 {
            return None;
        }

        if bar & 0x1 != 0 {
            let addr = bar & !0x3;
            return Some(BaseAddress::MmioAddressSpace(addr));
        }

        let bar_type = (bar >> 1) & 0x3;
        let prefetchable = bar & 0x8 != 0;
        let phys_addr = PhysicalAddress::new((bar & !0xf) as u64);

        match bar_type {
            0x0 => Some(BaseAddress::MemoryAddress32BitSpace(
                phys_addr,
                prefetchable,
            )),
            0x2 => Some(BaseAddress::MemoryAddress64BitSpace(
                phys_addr,
                prefetchable,
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ConfigurationSpaceNonBridgeField {
    bar0: BaseAddressRegister,
    bar1: BaseAddressRegister,
    bar2: BaseAddressRegister,
    bar3: BaseAddressRegister,
    bar4: BaseAddressRegister,
    bar5: BaseAddressRegister,
    cardbus_cis_ptr: u32,
    subsystem_vendor_id: u16,
    pub subsystem_id: u16,
    expansion_rom_base_addr: u32,
    pub caps_ptr: u8,
    reserved0: [u8; 3],
    reserved1: u32,
    int_line: u8,
    int_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

impl ConfigurationSpaceNonBridgeField {
    pub fn read(bus: usize, device: usize, func: usize) -> Result<Self> {
        let mut data: [u32; 12] = [0; 12];
        for (i, elem) in data.iter_mut().enumerate() {
            let d = read_conf_space(bus, device, func, PCI_CONF_UNIQUE_FIELD_OFFSET + (i * 4))?;
            *elem = d;
        }

        Ok(unsafe { transmute::<[u32; 12], Self>(data) })
    }

    pub fn get_bars(&self) -> Result<Vec<(usize, BaseAddress)>> {
        let mut skip_index = None;
        let bars = [
            &self.bar0, &self.bar1, &self.bar2, &self.bar3, &self.bar4, &self.bar5,
        ];
        let mut result = Vec::new();
        for (i, bar) in bars.iter().enumerate() {
            if let Some(skip) = skip_index {
                if skip == i {
                    skip_index = None;
                    continue;
                }
            }

            if let Some(base_addr) = bar.get_base_addr() {
                match base_addr {
                    BaseAddress::MemoryAddress64BitSpace(phys_addr, is_pref) => {
                        if i + 1 == bars.len() {
                            unreachable!()
                        }

                        let next_bar = bars[i + 1];
                        let full_phys_addr: PhysicalAddress =
                            ((next_bar.read() as u64) << 32 | phys_addr.get()).into();
                        skip_index = Some(i + 1);

                        if full_phys_addr.get() == 0 {
                            continue;
                        }

                        let start = full_phys_addr.get().into();
                        // TODO: implement get bar size
                        paging::update_mapping(&MappingInfo {
                            start,
                            end: start.offset(PAGE_SIZE * 3).into(),
                            phys_addr: full_phys_addr,
                            rw: ReadWrite::Write,
                            us: EntryMode::Supervisor,
                            pwt: PageWriteThroughLevel::WriteThrough,
                            pcd: true, // page cache disable
                        })?;

                        let base_addr =
                            BaseAddress::MemoryAddress64BitSpace(full_phys_addr, is_pref);
                        result.push((i, base_addr));
                    }
                    BaseAddress::MemoryAddress32BitSpace(phys_addr, _) => {
                        if phys_addr.get() == 0 {
                            continue;
                        }

                        let start = phys_addr.get().into();
                        // TODO: implement get bar size
                        paging::update_mapping(&MappingInfo {
                            start,
                            end: (start.get() + (PAGE_SIZE * 3) as u64).into(),
                            phys_addr,
                            rw: ReadWrite::Write,
                            us: EntryMode::Supervisor,
                            pwt: PageWriteThroughLevel::WriteThrough,
                            pcd: true, // page cache disable
                        })?;
                        result.push((i, base_addr));
                    }
                    _ => result.push((i, base_addr)),
                }
            }
        }

        Ok(result)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ConfigurationSpacePciToPciBridgeField {
    pub bar0: BaseAddressRegister,
    pub bar1: BaseAddressRegister,
    primary_bus_num: u8,
    secondary_bus_num: u8,
    subordinate_bus_num: u8,
    secondary_latency_timer: u8,
    io_base_low: u8,
    io_limit_low: u8,
    seconday_status: u16,
    mem_base: u16,
    mem_limit: u16,
    pref_mem_base_low: u16,
    pref_mem_limit_low: u16,
    pref_mem_base_high: u32,
    pref_mem_limit_high: u32,
    io_base_high: u16,
    io_limit_high: u16,
    pub caps_ptr: u8,
    reserved: [u8; 3],
    expansion_rom_base_addr: u32,
    int_line: u8,
    int_pin: u8,
    bridge_ctrl: u16,
}

impl ConfigurationSpacePciToPciBridgeField {
    pub fn read(bus: usize, device: usize, func: usize) -> Result<Self> {
        let mut data: [u32; 12] = [0; 12];
        for (i, elem) in data.iter_mut().enumerate() {
            let d = read_conf_space(bus, device, func, PCI_CONF_UNIQUE_FIELD_OFFSET + (i * 4))?;
            *elem = d;
        }

        Ok(unsafe { transmute::<[u32; 12], Self>(data) })
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ConfigurationSpacePciToCardBusField {
    cardbus_socket_or_exca_base_addr: u32,
    caps_list_offset: u8,
    reserved: u8,
    secondary_status: u16,
    pci_bus_num: u8,
    cardbus_bus_num: u8,
    subordinate_bus_num: u8,
    cardbus_latency_timer: u8,
    mem_base_addr0: u32,
    mem_limit0: u32,
    mem_base_addr1: u32,
    mem_limit1: u32,
    io_base_addr0: u32,
    io_limit0: u32,
    io_base_addr1: u32,
    io_limit1: u32,
    int_line: u8,
    int_pin: u8,
    bridge_ctrl: u16,
    subsystem_device_id: u16,
    subsystem_vendor_id: u16,
    pc_card_legacy_mode_base_addr: u32,
}

impl ConfigurationSpacePciToCardBusField {
    pub fn read(bus: usize, device: usize, func: usize) -> Result<Self> {
        let mut data: [u32; 14] = [0; 14];
        for (i, elem) in data.iter_mut().enumerate() {
            let d = read_conf_space(bus, device, func, PCI_CONF_UNIQUE_FIELD_OFFSET + (i * 4))?;
            *elem = d;
        }

        Ok(unsafe { transmute::<[u32; 14], Self>(data) })
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct MsiMessageControlField(u16);

impl MsiMessageControlField {
    pub fn set_is_enable(&mut self, value: bool) {
        self.0 = (self.0 & !0x1) | (value as u16);
    }

    pub fn set_multiple_msg_enable(&mut self, value: u8) {
        let value = value & 0x7; // 3 bits
        self.0 = (self.0 & !0x70) | ((value as u16) << 4);
    }

    pub fn raw(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct MsiCapabilityField {
    pub cap_id: u8,
    pub next_ptr: u8,
    pub msg_ctrl: MsiMessageControlField,
    pub msg_addr_low: MsiMessageAddressField,
    pub msg_addr_high: u32,
    pub msg_data: MsiMessageDataField,
    reserved: u64,
}

impl MsiCapabilityField {
    pub fn read(bus: usize, device: usize, func: usize, caps_ptr: usize) -> Result<Self> {
        let mut data: [u32; 6] = [0; 6];
        for (i, elem) in data.iter_mut().enumerate() {
            let d = read_conf_space(bus, device, func, caps_ptr + (i * 4))?;
            *elem = d;
        }

        Ok(unsafe { transmute::<[u32; 6], Self>(data) })
    }

    pub fn write(&self, bus: usize, device: usize, func: usize, caps_ptr: usize) -> Result<()> {
        let data = unsafe { transmute::<Self, [u32; 6]>(*self) };
        for (i, elem) in data.iter().enumerate() {
            if let Err(msg) = write_conf_space(bus, device, func, caps_ptr + (i * 4), *elem) {
                return Err(msg);
            }
        }

        Ok(())
    }
}

pub fn read_conf_space(bus: usize, device: usize, func: usize, byte_offset: usize) -> Result<u32> {
    if bus >= PCI_DEVICE_BUS_LEN
        || device >= PCI_DEVICE_DEVICE_LEN
        || func >= PCI_DEVICE_FUNC_LEN
        || byte_offset % 4 != 0
    {
        return Err(Error::Failed("Invalid args"));
    }

    let addr = 0x80000000
        | (bus as u32) << 16
        | (device as u32) << 11
        | (func as u32) << 8
        | byte_offset as u32;
    PCI_PORT_CONF_REG_ADDR.out32(addr);

    Ok(PCI_PORT_CONF_DATA_REG_ADDR.in32())
}

pub fn write_conf_space(
    bus: usize,
    device: usize,
    func: usize,
    byte_offset: usize,
    data: u32,
) -> Result<()> {
    if bus >= PCI_DEVICE_BUS_LEN
        || device >= PCI_DEVICE_DEVICE_LEN
        || func >= PCI_DEVICE_FUNC_LEN
        || byte_offset % 4 != 0
    {
        return Err(Error::Failed("Invalid args"));
    }

    let addr = 0x80000000
        | (bus as u32) << 16
        | (device as u32) << 11
        | (func as u32) << 8
        | byte_offset as u32;
    PCI_PORT_CONF_REG_ADDR.out32(addr);
    PCI_PORT_CONF_DATA_REG_ADDR.out32(data);

    Ok(())
}
