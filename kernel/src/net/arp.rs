use crate::{
    error::{Error, Result},
    net::eth::*,
};
use alloc::vec::Vec;
use core::{fmt::Debug, net::Ipv4Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpOperation {
    Request,
    Reply,
}

impl From<ArpOperation> for [u8; 2] {
    fn from(value: ArpOperation) -> [u8; 2] {
        match value {
            ArpOperation::Request => [0, 1],
            ArpOperation::Reply => [0, 2],
        }
    }
}

impl TryFrom<[u8; 2]> for ArpOperation {
    type Error = Error;

    fn try_from(value: [u8; 2]) -> Result<Self> {
        match value {
            [0, 1] => Ok(ArpOperation::Request),
            [0, 2] => Ok(ArpOperation::Reply),
            _ => Err(Error::Failed("Invalid ARP operation")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArpPacket {
    hardware_ty: [u8; 2], // must be [0, 1]
    protocol_ty: EthernetType,
    hardware_len: u8, // must be 6
    protocol_len: u8, // must be 4
    op: [u8; 2],
    pub sender_eth_addr: EthernetAddress,
    pub sender_ipv4_addr: Ipv4Addr,
    pub target_eth_addr: EthernetAddress,
    pub target_ipv4_addr: Ipv4Addr,
}

impl TryFrom<&[u8]> for ArpPacket {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self> {
        if data.len() < 28 {
            return Err(Error::Failed("Invalid data length"));
        }

        let hardware_ty = [data[0], data[1]];
        let protocol_ty = EthernetType::from([data[2], data[3]]);
        let hardware_len = data[4];
        let protocol_len = data[5];
        let op = [data[6], data[7]];
        let sender_eth_addr =
            EthernetAddress::from([data[8], data[9], data[10], data[11], data[12], data[13]]);
        let sender_ipv4_addr = Ipv4Addr::new(data[14], data[15], data[16], data[17]);
        let target_eth_addr =
            EthernetAddress::from([data[18], data[19], data[20], data[21], data[22], data[23]]);
        let target_ipv4_addr = Ipv4Addr::new(data[24], data[25], data[26], data[27]);

        Ok(Self {
            hardware_ty,
            protocol_ty,
            hardware_len,
            protocol_len,
            op,
            sender_eth_addr,
            sender_ipv4_addr,
            target_eth_addr,
            target_ipv4_addr,
        })
    }
}

impl ArpPacket {
    pub fn new_with(
        op: ArpOperation,
        sender_eth_addr: EthernetAddress,
        sender_ipv4_addr: Ipv4Addr,
        target_eth_addr: EthernetAddress,
        target_ipv4_addr: Ipv4Addr,
    ) -> Self {
        Self {
            hardware_ty: [0, 1],
            protocol_ty: EthernetType::Ipv4,
            hardware_len: 6,
            protocol_len: 4,
            op: op.into(),
            sender_eth_addr,
            sender_ipv4_addr,
            target_eth_addr,
            target_ipv4_addr,
        }
    }

    pub fn op(&self) -> Result<ArpOperation> {
        ArpOperation::try_from(self.op)
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let protocol_ty: [u8; 2] = self.protocol_ty.into();
        let sender_eth_addr: [u8; 6] = self.sender_eth_addr.into();
        let target_eth_addr: [u8; 6] = self.target_eth_addr.into();

        let mut vec = Vec::new();
        vec.extend_from_slice(&self.hardware_ty);
        vec.extend_from_slice(&protocol_ty);
        vec.push(self.hardware_len);
        vec.push(self.protocol_len);
        vec.extend_from_slice(&self.op);
        vec.extend_from_slice(&sender_eth_addr);
        vec.extend_from_slice(&self.sender_ipv4_addr.octets());
        vec.extend_from_slice(&target_eth_addr);
        vec.extend_from_slice(&self.target_ipv4_addr.octets());
        vec
    }
}
