use crate::error::{Error, Result};
use alloc::vec::Vec;
use core::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpSocketState {
    Closed,
    Listen,
    SynReceived,
    SynSent,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

#[derive(Debug)]
pub struct TcpSocket {
    state: TcpSocketState,
    src_port: Option<u16>,
    dst_ipv4_addr: Option<Ipv4Addr>,
    dst_port: Option<u16>,
    seq_num: u32,
    next_recv_seq: u32,
    buf: Vec<u8>,
}

impl TcpSocket {
    pub fn new() -> Self {
        Self {
            state: TcpSocketState::Closed,
            src_port: None,
            dst_ipv4_addr: None,
            dst_port: None,
            seq_num: 0,
            next_recv_seq: 0,
            buf: Vec::new(),
        }
    }

    pub fn state(&self) -> TcpSocketState {
        self.state
    }

    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }

    pub fn next_recv_seq(&self) -> u32 {
        self.next_recv_seq
    }

    pub fn get_and_reset_buf(&mut self) -> Vec<u8> {
        let buf = self.buf.clone();
        self.buf = Vec::new();
        buf
    }

    // server mode
    pub fn start_passive(&mut self, src_port: u16) -> Result<()> {
        if self.state != TcpSocketState::Closed {
            return Err("Invalid state".into());
        }

        self.state = TcpSocketState::Listen;
        self.src_port = Some(src_port);
        self.seq_num = 0;
        let _ = self.get_and_reset_buf();

        Ok(())
    }

    // client mode
    pub fn start_active(&mut self, dst_ipv4_addr: Ipv4Addr, dst_port: u16) -> Result<()> {
        if self.state != TcpSocketState::Closed {
            return Err("Invalid state".into());
        }

        self.state = TcpSocketState::SynSent;
        self.dst_ipv4_addr = Some(dst_ipv4_addr);
        self.dst_port = Some(dst_port);
        self.seq_num = 0;
        let _ = self.get_and_reset_buf();

        Ok(())
    }

    pub fn receive_syn(&mut self, remote_seq: u32) -> Result<u32> {
        if self.state != TcpSocketState::Listen {
            return Err("Invalid state".into());
        }

        self.state = TcpSocketState::SynReceived;
        self.next_recv_seq = remote_seq.wrapping_add(1);
        let isn = self.seq_num;
        self.seq_num = self.seq_num.wrapping_add(1);
        Ok(isn)
    }

    pub fn receive_ack(&mut self) -> Result<()> {
        if self.state != TcpSocketState::SynReceived && self.state != TcpSocketState::Established {
            return Err("Invalid state".into());
        }

        self.state = TcpSocketState::Established;
        Ok(())
    }

    pub fn receive_fin(&mut self) -> Result<()> {
        if self.state != TcpSocketState::Established {
            return Err("Invalid state".into());
        }

        self.state = TcpSocketState::CloseWait;
        self.next_recv_seq = self.next_recv_seq.wrapping_add(1);
        Ok(())
    }

    pub fn receive_data(&mut self, data: &[u8], seq_num: u32) -> Result<()> {
        if self.state != TcpSocketState::Established {
            return Err("Invalid state".into());
        }

        if seq_num != self.next_recv_seq {
            // Out of order packet, ignore for now
            return Ok(());
        }

        if !data.is_empty() {
            self.buf.extend_from_slice(data);
            self.next_recv_seq = self.next_recv_seq.wrapping_add(data.len() as u32);
        }

        Ok(())
    }

    pub fn close(&mut self) {
        *self = Self::new();
    }
}

#[derive(Debug, Clone)]
pub struct TcpPacket {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    urgent_ptr: u16,
    pub options_and_data: Vec<u8>,
}

impl TryFrom<&[u8]> for TcpPacket {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() < 20 {
            return Err("Invalid data length".into());
        }

        let src_port = u16::from_be_bytes([value[0], value[1]]);
        let dst_port = u16::from_be_bytes([value[2], value[3]]);
        let seq_num = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
        let ack_num = u32::from_be_bytes([value[8], value[9], value[10], value[11]]);
        let flags = u16::from_be_bytes([value[12], value[13]]);
        let window_size = u16::from_be_bytes([value[14], value[15]]);
        let checksum = u16::from_be_bytes([value[16], value[17]]);
        let urgent_ptr = u16::from_be_bytes([value[18], value[19]]);

        let data_offset_words = (flags >> 12) as usize;
        if data_offset_words < 5 {
            return Err("Invalid TCP data offset".into());
        }
        let header_len = data_offset_words * 4;
        if value.len() < header_len {
            return Err("Packet shorter than header length".into());
        }

        let options_and_data = value[20..].to_vec();

        Ok(Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            flags,
            window_size,
            checksum,
            urgent_ptr,
            options_and_data,
        })
    }
}

impl TcpPacket {
    pub const FLAGS_FIN: u16 = 0x01;
    pub const FLAGS_SYN: u16 = 0x02;
    pub const FLAGS_RST: u16 = 0x04;
    pub const FLAGS_PSH: u16 = 0x08;
    pub const FLAGS_ACK: u16 = 0x10;
    pub const FLAGS_URG: u16 = 0x20;
    pub const FLAGS_ECE: u16 = 0x40;
    pub const FLAGS_CWR: u16 = 0x80;
    pub const FLAGS_NS: u16 = 0x100;

    pub fn new_with(
        src_port: u16,
        dst_port: u16,
        seq_num: u32,
        ack_num: u32,
        flags_without_header_len: u16,
        window_size: u16,
        urgent_ptr: u16,
        mut options_and_data: Vec<u8>,
    ) -> Self {
        let header_len = ((20 + options_and_data.len() + 3) / 4) as u16;
        let flags = header_len << 12 | flags_without_header_len & 0x0fff;

        // resize options
        options_and_data.resize((header_len as usize * 4 - 20) as usize, 0);

        Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            flags,
            window_size,
            checksum: 0,
            urgent_ptr,
            options_and_data,
        }
    }

    pub fn calc_checksum(&mut self) {
        self.checksum = 0;
        let mut sum: u32 = 0;

        let packet = self.to_vec();
        for chunk in packet.chunks(2) {
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

    pub fn verify_checksum_with_ipv4(&self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) -> bool {
        let mut sum: u32 = 0;

        // pseudo header
        let src_octets = src_addr.octets();
        sum += ((src_octets[0] as u32) << 8) | (src_octets[1] as u32);
        sum += ((src_octets[2] as u32) << 8) | (src_octets[3] as u32);

        let dst_octets = dst_addr.octets();
        sum += ((dst_octets[0] as u32) << 8) | (dst_octets[1] as u32);
        sum += ((dst_octets[2] as u32) << 8) | (dst_octets[3] as u32);

        sum += 6; // protocol number (TCP = 6)

        // TCP header and data checksum
        let packet = self.to_vec();
        let tcp_len = packet.len();
        sum += tcp_len as u32;
        for chunk in packet.chunks(2) {
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

        let checksum = !(sum as u16);
        checksum == 0xffff || checksum == 0
    }

    pub fn calc_checksum_with_ipv4(&mut self, src_addr: Ipv4Addr, dst_addr: Ipv4Addr) {
        self.checksum = 0;
        let mut sum: u32 = 0;

        // pseudo header
        let src_octets = src_addr.octets();
        sum += ((src_octets[0] as u32) << 8) | (src_octets[1] as u32);
        sum += ((src_octets[2] as u32) << 8) | (src_octets[3] as u32);

        let dst_octets = dst_addr.octets();
        sum += ((dst_octets[0] as u32) << 8) | (dst_octets[1] as u32);
        sum += ((dst_octets[2] as u32) << 8) | (dst_octets[3] as u32);

        sum += 6; // protocol number (TCP = 6)

        // TCP header and data checksum
        let packet = self.to_vec();
        let tcp_len = packet.len();
        sum += tcp_len as u32;

        for chunk in packet.chunks(2) {
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

    pub fn flags_header_len(&self) -> usize {
        (self.flags >> 12) as usize * 4
    }

    pub fn flags_fin(&self) -> bool {
        self.flags & Self::FLAGS_FIN != 0
    }

    pub fn flags_syn(&self) -> bool {
        self.flags & Self::FLAGS_SYN != 0
    }

    pub fn flags_rst(&self) -> bool {
        self.flags & Self::FLAGS_RST != 0
    }

    pub fn flags_psh(&self) -> bool {
        self.flags & Self::FLAGS_PSH != 0
    }

    pub fn flags_ack(&self) -> bool {
        self.flags & Self::FLAGS_ACK != 0
    }

    pub fn flags_urg(&self) -> bool {
        self.flags & Self::FLAGS_URG != 0
    }

    pub fn flags_ece(&self) -> bool {
        self.flags & Self::FLAGS_ECE != 0
    }

    pub fn flags_cwr(&self) -> bool {
        self.flags & Self::FLAGS_CWR != 0
    }

    pub fn flags_ns(&self) -> bool {
        self.flags & Self::FLAGS_NS != 0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::new();
        vec.extend_from_slice(&self.src_port.to_be_bytes());
        vec.extend_from_slice(&self.dst_port.to_be_bytes());
        vec.extend_from_slice(&self.seq_num.to_be_bytes());
        vec.extend_from_slice(&self.ack_num.to_be_bytes());
        vec.extend_from_slice(&self.flags.to_be_bytes());
        vec.extend_from_slice(&self.window_size.to_be_bytes());
        vec.extend_from_slice(&self.checksum.to_be_bytes());
        vec.extend_from_slice(&self.urgent_ptr.to_be_bytes());
        vec.extend_from_slice(&self.options_and_data);
        vec
    }
}
