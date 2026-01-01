use crate::{
    error::{Error, Result},
    util::{self, slice::Sliceable},
};
use core::{marker::PhantomPinned, ops::RangeInclusive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(unused)]
pub enum UsbDescriptorType {
    Device = 1,
    Config = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    Hid = 0x21,
    Report = 0x22,
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct UsbDeviceDescriptor {
    pub desc_len: u8,
    pub desc_type: u8,
    pub version: u16,
    pub dev_class: u8,
    pub dev_subclass: u8,
    pub dev_protocol: u8,
    pub max_packet_size: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_index: u8,
    pub num_of_config: u8,
}

unsafe impl Sliceable for UsbDeviceDescriptor {}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct ConfigDescriptor {
    desc_len: u8,
    desc_type: u8,
    total_len: u16,
    num_of_interfaces: u8,
    config_value: u8,
    conf_string_index: u8,
    attr: u8,
    max_power: u8,
    _pinned: PhantomPinned,
}

impl ConfigDescriptor {
    pub fn total_len(&self) -> usize {
        self.total_len as usize
    }

    pub fn config_value(&self) -> u8 {
        self.config_value
    }
}

unsafe impl Sliceable for ConfigDescriptor {}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct InterfaceDescriptor {
    desc_len: u8,
    desc_type: u8,
    pub interface_num: u8,
    pub alt_setting: u8,
    num_of_endpoints: u8,
    interface_class: u8,
    interface_subclass: u8,
    interface_protocol: u8,
    interface_index: u8,
}

unsafe impl Sliceable for InterfaceDescriptor {}

impl InterfaceDescriptor {
    pub fn triple(&self) -> (u8, u8, u8) {
        (
            self.interface_class,
            self.interface_subclass,
            self.interface_protocol,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct EndpointDescriptor {
    pub desc_len: u8,
    pub desc_type: u8,
    pub endpoint_addr: u8,
    pub attr: u8,
    pub max_packet_size: u16,
    pub interval: u8,
}

unsafe impl Sliceable for EndpointDescriptor {}

#[derive(Debug)]
#[repr(u8)]
#[allow(unused)]
pub enum UsbHidReportItemType {
    Main = 0,
    Global = 1,
    Local = 2,
    Reserved = 3,
}

#[derive(Debug, Clone, Copy)]
#[allow(unused)]
pub enum UsbHidUsagePage {
    GenericDesktop,
    Button,
    Unknown(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(unused)]
pub enum UsbHidUsage {
    Pointer,
    Mouse,
    X,
    Y,
    Wheel,
    Button(usize),
    Unknown(usize),
    Constant,
}

#[derive(Debug)]
pub struct UsbHidReportInputItem {
    pub usage: UsbHidUsage,
    pub bit_size: usize,
    pub is_array: bool,
    pub is_absolute: bool,
    pub bit_offset: usize,
    pub logical_min: u32,
    pub logical_max: u32,
}

impl UsbHidReportInputItem {
    pub fn value_from_report(&self, report: &[u8]) -> Option<i64> {
        util::bits::extract_bits_from_le_bytes(report, self.bit_offset, self.bit_size).map(|v| {
            if self.bit_size >= 2 && util::bits::extract_bits(v, self.bit_size - 1, 1) == 1 {
                -(!util::bits::extract_bits(v, 0, self.bit_size - 1) as i64) - 1
            } else {
                v as i64
            }
        })
    }

    pub fn mapped_range_from_report(
        &self,
        report: &[u8],
        to_range: RangeInclusive<i64>,
    ) -> Result<i64> {
        let value = self
            .value_from_report(report)
            .ok_or::<Error>("Value was empty".into())?;
        util::range::map_value_range_inclusive(
            (self.logical_min as i64)..=(self.logical_max as i64),
            to_range,
            value,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[allow(unused)]
#[repr(packed)]
pub struct HidDescriptor {
    desc_len: u8,
    desc_type: u8,
    hid_release: u16,
    country_code: u8,
    num_descs: u8,
    descriptor_type: u8,
    pub report_desc_len: u16,
}

unsafe impl Sliceable for HidDescriptor {}

#[derive(Debug, Clone, Copy)]
pub enum UsbDescriptor {
    Config(ConfigDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
    Hid(HidDescriptor),
    Unknown { desc_len: u8, desc_type: u8 },
}

pub struct DescriptorIterator<'a> {
    buf: &'a [u8],
    index: usize,
}

impl<'a> DescriptorIterator<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, index: 0 }
    }
}

impl<'a> Iterator for DescriptorIterator<'a> {
    type Item = UsbDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buf.len() {
            None
        } else {
            let buf = &self.buf[self.index..];
            let desc_len = buf[0];
            let desc_type = buf[1];
            let desc = match desc_type {
                e if e == UsbDescriptorType::Config as u8 => {
                    UsbDescriptor::Config(ConfigDescriptor::copy_from_slice(buf).ok()?)
                }
                e if e == UsbDescriptorType::Interface as u8 => {
                    UsbDescriptor::Interface(InterfaceDescriptor::copy_from_slice(buf).ok()?)
                }
                e if e == UsbDescriptorType::Endpoint as u8 => {
                    UsbDescriptor::Endpoint(EndpointDescriptor::copy_from_slice(buf).ok()?)
                }
                e if e == UsbDescriptorType::Hid as u8 => {
                    UsbDescriptor::Hid(HidDescriptor::copy_from_slice(buf).ok()?)
                }
                _ => UsbDescriptor::Unknown {
                    desc_len,
                    desc_type,
                },
            };
            self.index += desc_len as usize;
            Some(desc)
        }
    }
}
