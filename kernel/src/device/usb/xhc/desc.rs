use crate::{sync::pin::IntoPinnedMutableSlice, util::slice::Sliceable};
use core::marker::PhantomPinned;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(unused)]
pub enum UsbDescriptorType {
    Device = 1,
    Config = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
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

unsafe impl IntoPinnedMutableSlice for UsbDeviceDescriptor {}

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

unsafe impl IntoPinnedMutableSlice for ConfigDescriptor {}
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

unsafe impl IntoPinnedMutableSlice for InterfaceDescriptor {}
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

unsafe impl IntoPinnedMutableSlice for EndpointDescriptor {}
unsafe impl Sliceable for EndpointDescriptor {}

#[derive(Debug, Clone, Copy)]
pub enum UsbDescriptor {
    Config(ConfigDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
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
