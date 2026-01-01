use crate::error::Error;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::net::Ipv4Addr;

#[derive(Debug)]
pub struct UdpSocket {
    buf: Vec<u8>,
}

impl UdpSocket {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn receive(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn buf_to_string_utf8_lossy(&self) -> String {
        String::from_utf8_lossy(&self.buf).to_string()
    }

    pub fn read_buf(&mut self, buf: &mut [u8]) -> usize {
        let read_len = buf.len().min(self.buf.len());
        if read_len > 0 {
            buf[..read_len].copy_from_slice(&self.buf[..read_len]);
            self.buf.drain(..read_len);
        }
        read_len
    }
}

#[derive(Debug, Clone)]
pub struct UdpPacket {
    src_port: u16,
    pub dst_port: u16,
    len: u16,
    checksum: u16,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for UdpPacket {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 8 {
            return Err("Invalid data length".into());
        }

        let src_port = u16::from_be_bytes([value[0], value[1]]);
        let dst_port = u16::from_be_bytes([value[2], value[3]]);
        let len = u16::from_be_bytes([value[4], value[5]]);
        let checksum = u16::from_be_bytes([value[6], value[7]]);
        let data = value[8..(len as usize)].to_vec();

        Ok(Self {
            src_port,
            dst_port,
            len,
            checksum,
            data,
        })
    }
}

impl UdpPacket {
    pub fn new_with(src_port: u16, dst_port: u16, data: &[u8]) -> Self {
        let len = 8 + data.len() as u16;

        Self {
            src_port,
            dst_port,
            len,
            checksum: 0,
            data: data.to_vec(),
        }
    }

    pub fn calc_checksum_with_ipv4(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) {
        self.checksum = 0;
        let mut sum: u32 = 0;

        // pseudo header
        let src = src_addr.octets();
        let dst = dst_addr.octets();
        sum += ((src[0] as u32) << 8) | (src[1] as u32);
        sum += ((src[2] as u32) << 8) | (src[3] as u32);
        sum += ((dst[0] as u32) << 8) | (dst[1] as u32);
        sum += ((dst[2] as u32) << 8) | (dst[3] as u32);
        sum += 17;

        let udp_vec = self.to_vec();
        let udp_len = udp_vec.len() as u32;
        sum += udp_len;

        for chunk in udp_vec.chunks(2) {
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
        vec.extend_from_slice(&self.src_port.to_be_bytes());
        vec.extend_from_slice(&self.dst_port.to_be_bytes());
        vec.extend_from_slice(&self.len.to_be_bytes());
        vec.extend_from_slice(&self.checksum.to_be_bytes());
        vec.extend_from_slice(&self.data);
        vec
    }
}
