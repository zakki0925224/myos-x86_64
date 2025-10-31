use crate::{
    error::{Error, Result},
    net::{arp::ArpPacket, ip::Ipv4Packet},
};
use alloc::vec::Vec;
use core::fmt::Debug;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EthernetAddress([u8; 6]);

impl Debug for EthernetAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mac = self.0;

        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        )
    }
}

impl From<[u8; 6]> for EthernetAddress {
    fn from(data: [u8; 6]) -> Self {
        Self(data)
    }
}

impl From<EthernetAddress> for [u8; 6] {
    fn from(addr: EthernetAddress) -> Self {
        addr.0
    }
}

impl EthernetAddress {
    pub fn broadcast() -> Self {
        Self::from([0xff; 6])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthernetType {
    Ipv4,
    Ipv6,
    Arp,
    Other(u16),
    PayloadLength(u16),
}

impl From<[u8; 2]> for EthernetType {
    fn from(data: [u8; 2]) -> Self {
        let value = u16::from_be_bytes(data);

        if value <= 0x05dc {
            EthernetType::PayloadLength(value)
        } else {
            match value {
                0x0800 => EthernetType::Ipv4,
                0x86dd => EthernetType::Ipv6,
                0x0806 => EthernetType::Arp,
                _ => EthernetType::Other(value),
            }
        }
    }
}

impl From<EthernetType> for [u8; 2] {
    fn from(value: EthernetType) -> Self {
        match value {
            EthernetType::Ipv4 => [0x08, 0x00],
            EthernetType::Ipv6 => [0x86, 0xdd],
            EthernetType::Arp => [0x08, 0x06],
            EthernetType::Other(value) => value.to_be_bytes(),
            EthernetType::PayloadLength(value) => value.to_be_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum EthernetPayload {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
    None,
}

impl EthernetPayload {
    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            EthernetPayload::Arp(packet) => packet.to_vec(),
            EthernetPayload::Ipv4(packet) => packet.to_vec(),
            EthernetPayload::None => Vec::new(),
        }
    }
}

pub struct EthernetFrame {
    pub dst_mac_addr: EthernetAddress,
    pub src_mac_addr: EthernetAddress,
    pub eth_type: EthernetType,
    payload: Vec<u8>,
    // fcs: u32,
}

impl Debug for EthernetFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EthernetFrame")
            .field("dst_mac_addr", &self.dst_mac_addr)
            .field("src_mac_addr", &self.src_mac_addr)
            .field("eth_type", &self.eth_type)
            .field("payload", &self.payload())
            .finish()
    }
}

impl TryFrom<&[u8]> for EthernetFrame {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() < 14 {
            return Err("Invalid data length".into());
        }

        let dst_mac = &value[0..6];
        let src_mac = &value[6..12];
        let eth_type = [value[12], value[13]].into();
        let payload = value[14..].to_vec();

        Ok(Self {
            dst_mac_addr: [
                dst_mac[0], dst_mac[1], dst_mac[2], dst_mac[3], dst_mac[4], dst_mac[5],
            ]
            .into(),
            src_mac_addr: [
                src_mac[0], src_mac[1], src_mac[2], src_mac[3], src_mac[4], src_mac[5],
            ]
            .into(),
            eth_type,
            payload,
        })
    }
}

impl EthernetFrame {
    pub fn new_with(
        dst_mac_addr: EthernetAddress,
        src_mac_addr: EthernetAddress,
        eth_type: EthernetType,
        payload: &[u8],
    ) -> Self {
        Self {
            dst_mac_addr,
            src_mac_addr,
            eth_type,
            payload: payload.to_vec(),
        }
    }

    pub fn to_vec(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        let dst_mac_addr: [u8; 6] = self.dst_mac_addr.into();
        let src_mac_addr: [u8; 6] = self.src_mac_addr.into();
        let eth_type: [u8; 2] = self.eth_type.into();

        let payload = self.payload()?.to_vec();
        let payload_len = payload.len().max(46);
        let frame_len = (14 + payload_len).max(64);

        vec.extend_from_slice(&dst_mac_addr);
        vec.extend_from_slice(&src_mac_addr);
        vec.extend_from_slice(&eth_type);
        vec.extend_from_slice(&payload);

        // padding
        vec.resize(frame_len, 0);

        Ok(vec)
    }

    pub fn payload(&self) -> Result<EthernetPayload> {
        let payload = match self.eth_type {
            EthernetType::Arp => {
                EthernetPayload::Arp(ArpPacket::try_from(self.payload.as_slice())?)
            }
            EthernetType::Ipv4 => {
                EthernetPayload::Ipv4(Ipv4Packet::try_from(self.payload.as_slice())?)
            }
            _ => EthernetPayload::None,
        };
        Ok(payload)
    }
}
