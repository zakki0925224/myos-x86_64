use crate::{
    error::{Error, Result},
    kdebug, kinfo, kwarn,
    net::{arp::*, eth::*, icmp::*, ip::*, socket::*, tcp::*, udp::*},
    sync::mutex::Mutex,
};
use alloc::{collections::btree_map::BTreeMap, string::String, vec::Vec};
use core::net::Ipv4Addr;

pub mod arp;
pub mod eth;
pub mod icmp;
pub mod ip;
pub mod socket;
pub mod tcp;
pub mod udp;

type ArpTable = BTreeMap<Ipv4Addr, EthernetAddress>;

static mut NETWORK_MAN: Mutex<NetworkManager> =
    Mutex::new(NetworkManager::new(Ipv4Addr::new(192, 168, 100, 2)));

struct NetworkManager {
    my_ipv4_addr: Ipv4Addr,
    my_mac_addr: Option<EthernetAddress>,
    arp_table: ArpTable,
    socket_table: SocketTable,
}

impl NetworkManager {
    const fn new(ipv4_addr: Ipv4Addr) -> Self {
        Self {
            my_ipv4_addr: ipv4_addr,
            my_mac_addr: None,
            arp_table: ArpTable::new(),
            socket_table: SocketTable::new(),
        }
    }

    fn set_my_mac_addr(&mut self, mac_addr: EthernetAddress) {
        self.my_mac_addr = Some(mac_addr);

        kinfo!("net: MAC address set to {:?}", mac_addr);
        kinfo!("net: IP address: {:?}", self.my_ipv4_addr);
    }

    fn my_mac_addr(&self) -> Result<EthernetAddress> {
        self.my_mac_addr
            .ok_or(Error::Failed("MAC address is not set"))
    }

    fn insert_new_socket(&mut self, type_: SocketType) -> Result<SocketId> {
        let protocol = match type_ {
            SocketType::Stream => Protocol::Tcp,
            SocketType::Dgram => Protocol::Tcp,
        };

        self.socket_table.insert_new_socket(type_, protocol)
    }

    fn udp_socket_mut(&mut self, port: u16) -> Result<&mut UdpSocket> {
        let type_ = SocketType::Dgram;

        let socket_id = if let Ok(id) = self.socket_table.socket_id_by_port_and_type(port, type_) {
            id
        } else {
            let id = self.socket_table.insert_new_socket(type_, Protocol::Udp)?;
            self.socket_table.bind_port(id, Some(port))?;
            id
        };

        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        socket.inner_udp_mut()
    }

    fn tcp_socket_mut(&mut self, port: u16) -> Result<&mut TcpSocket> {
        let type_ = SocketType::Stream;

        let socket_id = if let Ok(id) = self.socket_table.socket_id_by_port_and_type(port, type_) {
            id
        } else {
            let id = self.socket_table.insert_new_socket(type_, Protocol::Tcp)?;
            self.socket_table.bind_port(id, Some(port))?;
            id
        };

        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        socket.inner_tcp_mut()
    }

    fn receive_icmp_packet(&mut self, packet: IcmpPacket) -> Result<Option<IcmpPacket>> {
        let ty = packet.ty;

        match ty {
            IcmpType::EchoRequest => {
                let mut reply_packet = packet.clone();
                reply_packet.ty = IcmpType::EchoReply;
                reply_packet.calc_checksum();
                return Ok(Some(reply_packet));
            }
            _ => (),
        }

        Ok(None)
    }

    fn receive_tcp_packet(&mut self, packet: TcpPacket) -> Result<Option<TcpPacket>> {
        kinfo!("net: TCP packet received");

        let src_port = packet.src_port;
        let dst_port = packet.dst_port;
        let seq_num = packet.seq_num;
        let socket_mut = self.tcp_socket_mut(dst_port)?;

        // TODO: Remove after
        if socket_mut.state() == TcpSocketState::Closed {
            socket_mut.start_passive(dst_port)?;
        }

        match socket_mut.state() {
            TcpSocketState::Closed => {
                kwarn!("net: TCP received but socket is closed");
            }
            TcpSocketState::Listen => {
                if !packet.flags_syn() {
                    kwarn!("net: TCP-SYN not received");
                    return Ok(None);
                }

                let next_seq_num = socket_mut.receive_syn(seq_num)?;
                let ack_num = socket_mut.next_recv_seq();

                let mut options = Vec::new();
                let mss_bytes_len = 1460u16;
                options.push(0x02); // MSS
                options.push(0x04); // MSS length
                options.push((mss_bytes_len >> 8) as u8); // MSS high byte
                options.push((mss_bytes_len & 0xff) as u8); // MSS low byte

                // send SYN-ACK
                let reply_packet = TcpPacket::new_with(
                    dst_port,
                    src_port,
                    next_seq_num,
                    ack_num,
                    TcpPacket::FLAGS_SYN | TcpPacket::FLAGS_ACK,
                    u16::MAX,
                    0,
                    options,
                );
                kdebug!("net: TCP-SYN-ACK packet: {:?}", reply_packet);
                return Ok(Some(reply_packet));
            }
            TcpSocketState::SynReceived => {
                if !packet.flags_ack() {
                    kwarn!("net: TCP-ACK not received");
                    return Ok(None);
                }

                socket_mut.receive_ack()?;
            }
            TcpSocketState::Established => {
                let mut ack_needed = false;

                if packet.flags_ack() {
                    socket_mut.receive_ack()?;
                }

                let header_len = packet.flags_header_len();
                let options_len = header_len.checked_sub(20).unwrap_or(0);
                let data = &packet.options_and_data[options_len..];

                if !data.is_empty() {
                    kdebug!(
                        "net: TCP data packet received: {:?}",
                        String::from_utf8_lossy(data)
                    );
                    socket_mut.receive_data(data, seq_num)?;
                    ack_needed = true;
                }

                if packet.flags_fin() {
                    kdebug!("net: TCP-FIN received");
                    socket_mut.receive_fin()?;
                    ack_needed = true;
                }

                if ack_needed {
                    let next_seq_num = socket_mut.seq_num();
                    let ack_num = socket_mut.next_recv_seq();

                    let reply_packet = TcpPacket::new_with(
                        dst_port,
                        src_port,
                        next_seq_num,
                        ack_num,
                        TcpPacket::FLAGS_ACK,
                        u16::MAX,
                        0,
                        Vec::new(),
                    );
                    kdebug!("net: TCP-ACK packet: {:?}", reply_packet);
                    return Ok(Some(reply_packet));
                }

                return Ok(None);
            }
            TcpSocketState::CloseWait => {
                // ignore received packets
                // must be close socket from app
                return Ok(None);
            }
            state => {
                kwarn!("net: Unsupported TCP state: {:?}", state);
            }
        }

        Ok(None)
    }

    fn receive_udp_packet(&mut self, packet: UdpPacket) -> Result<Option<UdpPacket>> {
        let dst_port = packet.dst_port;
        let socket_mut = self.udp_socket_mut(dst_port)?;
        socket_mut.receive(&packet.data);
        let s = socket_mut.buf_to_string_utf8_lossy();
        kdebug!("net: UDP data: {:?}", s);

        Ok(None)
    }

    fn receive_arp_packet(&mut self, packet: ArpPacket) -> Result<Option<ArpPacket>> {
        let arp_op = packet.op()?;
        let sender_ipv4_addr = packet.sender_ipv4_addr;
        let sender_mac_addr = packet.sender_eth_addr;
        let target_ipv4_addr = packet.target_ipv4_addr;

        match arp_op {
            ArpOperation::Request => {
                self.arp_table.insert(sender_ipv4_addr, sender_mac_addr);

                if target_ipv4_addr != self.my_ipv4_addr {
                    return Ok(None);
                }

                let reply_packet = ArpPacket::new_with(
                    ArpOperation::Reply,
                    self.my_mac_addr()?,
                    self.my_ipv4_addr,
                    sender_mac_addr,
                    sender_ipv4_addr,
                );

                Ok(Some(reply_packet))
            }
            ArpOperation::Reply => {
                unimplemented!()
            }
        }
    }

    fn receive_ipv4_packet(&mut self, packet: Ipv4Packet) -> Result<Option<Ipv4Packet>> {
        packet.validate()?;

        if packet.dst_addr != self.my_ipv4_addr {
            return Ok(None);
        }

        let mut reply_payload = None;
        match packet.payload()? {
            Ipv4Payload::Icmp(icmp_packet) => {
                if let Some(reply_icmp_packet) = self.receive_icmp_packet(icmp_packet)? {
                    reply_payload = Some(Ipv4Payload::Icmp(reply_icmp_packet));
                }
            }
            Ipv4Payload::Tcp(tcp_packet) => {
                let is_valid =
                    tcp_packet.verify_checksum_with_ipv4(packet.src_addr, packet.dst_addr);
                assert!(is_valid, "Invalid TCP checksum");

                if let Some(mut reply_tcp_packet) = self.receive_tcp_packet(tcp_packet)? {
                    reply_tcp_packet.calc_checksum_with_ipv4(self.my_ipv4_addr, packet.src_addr);
                    reply_payload = Some(Ipv4Payload::Tcp(reply_tcp_packet));
                }
            }
            Ipv4Payload::Udp(udp_packet) => {
                self.receive_udp_packet(udp_packet)?;
            }
        }

        let mut reply_packet = None;
        if let Some(reply_payload) = reply_payload {
            let mut ipv4_packet = Ipv4Packet::new_with(
                packet.version_ihl,
                packet.dscp_ecn,
                packet.id,
                packet.flags,
                packet.protocol,
                self.my_ipv4_addr,
                packet.src_addr,
                reply_payload,
            );
            ipv4_packet.calc_checksum();
            reply_packet = Some(ipv4_packet);
        }

        Ok(reply_packet)
    }

    fn receive_eth_payload(&mut self, payload: EthernetPayload) -> Result<Option<EthernetPayload>> {
        let mut reply_payload = None;

        match payload {
            EthernetPayload::Arp(arp_packet) => {
                if let Some(reply_arp_packet) = self.receive_arp_packet(arp_packet)? {
                    reply_payload = Some(EthernetPayload::Arp(reply_arp_packet));
                }
            }
            EthernetPayload::Ipv4(ipv4_packet) => {
                if let Some(reply_ipv4_packet) = self.receive_ipv4_packet(ipv4_packet)? {
                    reply_payload = Some(EthernetPayload::Ipv4(reply_ipv4_packet));
                }
            }
            EthernetPayload::None => (),
        }

        Ok(reply_payload)
    }
}

pub fn set_my_mac_addr(mac_addr: EthernetAddress) -> Result<()> {
    unsafe { NETWORK_MAN.try_lock() }?.set_my_mac_addr(mac_addr);
    Ok(())
}

pub fn my_mac_addr() -> Result<EthernetAddress> {
    unsafe { NETWORK_MAN.try_lock() }?.my_mac_addr()
}

pub fn receive_eth_payload(payload: EthernetPayload) -> Result<Option<EthernetPayload>> {
    unsafe { NETWORK_MAN.try_lock() }?.receive_eth_payload(payload)
}

pub fn insert_new_socket(type_: SocketType) -> Result<SocketId> {
    unsafe { NETWORK_MAN.try_lock() }?.insert_new_socket(type_)
}
