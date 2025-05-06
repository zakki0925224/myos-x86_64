use self::{
    config::ConfigurationDescriptor, device::DeviceDescriptor, endpoint::EndpointDescriptor,
    hid::HumanInterfaceDeviceDescriptor, interface::InterfaceDescriptor,
};
use alloc::vec::Vec;

pub mod config;
pub mod device;
pub mod endpoint;
pub mod hid;
pub mod interface;

#[derive(Debug, Clone)]
pub enum Descriptor {
    Device(DeviceDescriptor),
    Configuration(ConfigurationDescriptor),
    Endpoint(EndpointDescriptor),
    Interface(InterfaceDescriptor),
    HumanInterfaceDevice(HumanInterfaceDeviceDescriptor, Vec<DescriptorHeader>),
    Unsupported((DescriptorType, DescriptorHeader)),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DescriptorType {
    Device = 0x1,
    Configuration = 0x2,
    String = 0x3,
    Interface = 0x4,
    Endpoint = 0x5,
    DeviceQualifier = 0x6,
    OtherSpeedConfiguration = 0x7,
    InterfacePower = 0x8,
    Otg = 0x9,
    Debug = 0xa,
    InterfaceAssociation = 0xb,
    BinaryDeviceObjectStore = 0xf,
    DeviceCapability = 0x10,
    SuperspeedUsbEndpointCompanion = 0x30,
    SuperspeedIsochronousEndpointCompanion = 0x31,

    // HID
    HumanInterfaceDevice = 0x21,
    Report = 0x22,
    Physical = 0x23,

    UsbHub = 0x29,
}

impl Default for DescriptorType {
    fn default() -> Self {
        Self::Device
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(packed)]
pub struct DescriptorHeader {
    pub length: u8,
    pub ty: DescriptorType,
}
