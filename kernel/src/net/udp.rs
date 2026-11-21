use crate::{
    error::Error,
    net::checksum::{checksum_words, fold_checksum, pseudo_header_sum},
};
use alloc::{collections::VecDeque, vec::Vec};
use core::net::Ipv4Addr;

struct UdpDatagram {
    src_addr: Ipv4Addr,
    src_port: u16,
    data: Vec<u8>,
}

pub struct UdpSocket {
    queue: VecDeque<UdpDatagram>,
}

impl UdpSocket {
    const QUEUE_LEN: usize = 64;

    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push_queue(&mut self, src_addr: Ipv4Addr, src_port: u16, data: &[u8]) {
        self.queue.push_back(UdpDatagram {
            src_addr,
            src_port,
            data: data.to_vec(),
        });

        if self.queue.len() > Self::QUEUE_LEN {
            // drop latest queue
            let _ = self.queue.pop_back();
        }
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Option<(usize, Ipv4Addr, u16)> {
        let datag = self.queue.pop_front()?;
        let recv_len = buf.len().min(datag.data.len());

        buf[..recv_len].copy_from_slice(&datag.data[..recv_len]);
        Some((recv_len, datag.src_addr, datag.src_port))
    }
}

#[derive(Debug, Clone)]
pub struct UdpPacket {
    pub src_port: u16,
    pub dst_port: u16,
    len: u16,
    checksum: u16,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for UdpPacket {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 8 {
            return Err(Error::InvalidBufferSize {
                required: 8,
                actual: value.len(),
            });
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
        let udp_vec = self.to_vec();
        let sum = pseudo_header_sum(src_addr, dst_addr, 17, udp_vec.len())
            .wrapping_add(checksum_words(&udp_vec));
        self.checksum = fold_checksum(sum);
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
