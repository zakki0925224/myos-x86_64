use crate::error::{Error, Result};
use alloc::vec::Vec;
use core::fmt::Debug;

#[derive(Debug, Clone, Copy)]
pub enum IcmpType {
    EchoReply,
    EchoRequest,
    Other(u8),
}

impl From<IcmpType> for u8 {
    fn from(ty: IcmpType) -> Self {
        match ty {
            IcmpType::EchoReply => 0,
            IcmpType::EchoRequest => 8,
            IcmpType::Other(x) => x,
        }
    }
}

impl From<u8> for IcmpType {
    fn from(data: u8) -> Self {
        match data {
            0 => IcmpType::EchoReply,
            8 => IcmpType::EchoRequest,
            _ => IcmpType::Other(data),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IcmpPacket {
    pub ty: IcmpType,
    code: u8,
    checksum: u16,
    id: u16,
    seq: u16,
    data: Vec<u8>,
}

impl TryFrom<&[u8]> for IcmpPacket {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() < 8 {
            return Err(Error::Failed("Invalid data length"));
        }

        let ty = value[0].into();
        let code = value[1];
        let checksum = u16::from_be_bytes([value[2], value[3]]);
        let id = u16::from_be_bytes([value[4], value[5]]);
        let seq = u16::from_be_bytes([value[6], value[7]]);
        let data = value[8..].to_vec();

        Ok(Self {
            ty,
            code,
            checksum,
            id,
            seq,
            data,
        })
    }
}

impl IcmpPacket {
    pub fn calc_checksum(&mut self) {
        self.checksum = 0;
        let mut sum: u32 = 0;

        let header = [
            self.ty.into(),
            self.code,
            0,
            0, // checksum
            (self.id >> 8) as u8,
            (self.id & 0xff) as u8,
            (self.seq >> 8) as u8,
            (self.seq & 0xff) as u8,
        ];

        for chunk in header.chunks(2).chain(self.data.chunks(2)) {
            let word = match chunk {
                [h, l] => u16::from_be_bytes([*h, *l]),
                [h] => u16::from_be_bytes([*h, 0]),
                _ => 0,
            };
            sum = sum.wrapping_add(word as u32);
        }

        while (sum >> 16) > 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        self.checksum = !(sum as u16);
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.push(self.ty.into());
        vec.push(self.code);
        vec.extend_from_slice(&self.checksum.to_be_bytes());
        vec.extend_from_slice(&self.id.to_be_bytes());
        vec.extend_from_slice(&self.seq.to_be_bytes());
        vec.extend_from_slice(&self.data);
        vec
    }
}
