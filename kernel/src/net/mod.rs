use crate::{
    arch::x86_64,
    device,
    error::{Error, Result},
    kinfo, kwarn,
    net::{arp::*, eth::*, icmp::*, ip::*, socket::*, tcp::*, udp::*},
    sync::mutex::Mutex,
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use core::{net::Ipv4Addr, time::Duration};

pub mod arp;
pub mod eth;
pub mod icmp;
pub mod ip;
pub mod socket;
pub mod tcp;
pub mod udp;

type ArpTable = BTreeMap<Ipv4Addr, (Option<EthernetAddress>, Duration)>;

const GATEWAY_ADDR: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
const LOCAL_ADDR: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
const SUBNET_MASK: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 0);

fn get_target_ip(my_ip: Ipv4Addr, dst_ip: Ipv4Addr) -> Ipv4Addr {
    let my_octets = my_ip.octets();
    let dst_octets = dst_ip.octets();
    let mask_octets = SUBNET_MASK.octets();

    let is_same_subnet =
        (0..4).all(|i| (my_octets[i] & mask_octets[i]) == (dst_octets[i] & mask_octets[i]));

    if is_same_subnet {
        dst_ip
    } else {
        GATEWAY_ADDR
    }
}

static NETWORK_MAN: Mutex<NetworkManager> = Mutex::new(NetworkManager::new(LOCAL_ADDR));

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
        self.my_mac_addr.ok_or("MAC address is not set".into())
    }

    fn create_new_socket(&mut self, type_: SocketType) -> Result<SocketId> {
        let protocol = match type_ {
            SocketType::Stream => Protocol::Tcp,
            SocketType::Dgram => Protocol::Udp,
        };

        let socket_id = self.socket_table.insert_new_socket(type_, protocol)?;
        kinfo!("net: Created new socket at {} ({:?})", socket_id, protocol);

        Ok(socket_id)
    }

    fn close_socket(&mut self, socket_id: SocketId) -> Result<()> {
        let _ = self.send_tcp_fin(socket_id);
        self.socket_table.remove_socket(socket_id)?;
        kinfo!("net: Closed socket {}", socket_id);
        Ok(())
    }

    fn udp_socket_mut_by_port(&mut self, port: u16) -> Result<&mut UdpSocket> {
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

    fn tcp_socket_mut_by_port(
        &mut self,
        local_port: u16,
        remote_addr: Ipv4Addr,
        remote_port: u16,
    ) -> Result<&mut TcpSocket> {
        let type_ = SocketType::Stream;

        if let Some(id) =
            self.socket_table
                .find_tcp_socket_by_port_and_addr(local_port, remote_addr, remote_port)
        {
            let socket = self.socket_table.socket_mut_by_id(id)?;
            return socket.inner_tcp_mut();
        }

        let socket_id = if let Ok(id) = self
            .socket_table
            .socket_id_by_port_and_type(local_port, type_)
        {
            id
        } else {
            let id = self.socket_table.insert_new_socket(type_, Protocol::Tcp)?;
            self.socket_table.bind_port(id, Some(local_port))?;
            id
        };

        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        socket.inner_tcp_mut()
    }

    fn bind_socket_v4(
        &mut self,
        socket_id: SocketId,
        bound_addr: Option<Ipv4Addr>,
        port: Option<u16>,
    ) -> Result<()> {
        {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;
            if socket.port() != 0 {
                return Err("Socket already bound".into());
            }
        }

        self.socket_table.bind_port(socket_id, port)?;
        kinfo!(
            "net: Bound socket {} to address: {:?}, port: {:?}",
            socket_id,
            bound_addr,
            port
        );

        {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;
            socket.addr = bound_addr;
        }

        Ok(())
    }

    fn sendto_udp_v4(
        &mut self,
        socket_id: SocketId,
        dst_addr: Ipv4Addr,
        dst_port: u16,
        data: &[u8],
    ) -> Result<()> {
        let socket = self.socket_table.socket_by_id(socket_id)?;
        let src_port = socket.port();

        self.send_udp_packet(src_port, dst_port, dst_addr, data)
    }

    fn recvfrom_udp_v4(&mut self, socket_id: SocketId, buf: &mut [u8]) -> Result<usize> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let udp_socket = socket.inner_udp_mut()?;
        let read_len = udp_socket.read_buf(buf);
        Ok(read_len)
    }

    fn listen_tcp_v4(&mut self, socket_id: SocketId) -> Result<()> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let port = socket.port();

        if port == 0 {
            return Err("Socket must be bound before listen".into());
        }

        let tcp_socket = socket.inner_tcp_mut()?;
        tcp_socket.start_passive(port)?;

        kinfo!("net: TCP listen on port {}", port);
        Ok(())
    }

    fn accept_tcp_v4(&mut self, socket_id: SocketId) -> Result<SocketId> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;

        if tcp_socket.state() != TcpSocketState::Listen {
            return Err("Socket is not listening".into());
        }

        let server_port = socket.port();

        if let Some(client_socket_id) = self.socket_table.find_tcp_established_socket(server_port) {
            return Ok(client_socket_id);
        }

        Err("No incoming connection".into())
    }

    fn connect_tcp_v4(
        &mut self,
        socket_id: SocketId,
        dst_addr: Ipv4Addr,
        dst_port: u16,
    ) -> Result<()> {
        {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;

            if socket.port() == 0 {
                self.socket_table.bind_port(socket_id, None)?;
            }
        }

        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;
        tcp_socket.start_active(dst_addr, dst_port)?;

        kinfo!("net: TCP connect initiated to {}:{}", dst_addr, dst_port);
        Ok(())
    }

    fn send_tcp_syn(&mut self, socket_id: SocketId) -> Result<()> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let src_port = socket.port();
        let tcp_socket = socket.inner_tcp_mut()?;

        if tcp_socket.state() != TcpSocketState::SynSent {
            return Err("Socket is not in SynSent state".into());
        }

        let dst_port = tcp_socket
            .dst_port()
            .ok_or::<Error>("No destination port".into())?;
        let dst_addr = tcp_socket
            .dst_ipv4_addr()
            .ok_or::<Error>("No destination address".into())?;

        let mut syn_packet = TcpPacket::new_with(
            src_port,
            dst_port,
            tcp_socket.seq_num(),
            0,
            TcpPacket::FLAGS_SYN,
            u16::MAX,
            0,
            Vec::new(),
            Vec::new(),
        );
        syn_packet.calc_checksum_with_ipv4(self.my_ipv4_addr, dst_addr);

        let mut ipv4_packet = Ipv4Packet::new_with(
            0x45,
            0,
            0,
            0,
            Protocol::Tcp,
            self.my_ipv4_addr,
            dst_addr,
            Ipv4Payload::Tcp(syn_packet),
        );
        ipv4_packet.calc_checksum();

        let target_ip = get_target_ip(self.my_ipv4_addr, dst_addr);
        let dst_mac_addr = self
            .resolve_mac_addr(target_ip)?
            .ok_or::<Error>("Failed to resolve MAC address".into())?;
        self.send_eth_payload(
            EthernetPayload::Ipv4(ipv4_packet),
            dst_mac_addr,
            EthernetType::Ipv4,
        )?;

        Ok(())
    }

    fn send_tcp_fin(&mut self, socket_id: SocketId) -> Result<()> {
        let (src_port, dst_port, dst_addr, seq_num, ack_num) = {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;
            let src_port = socket.port();
            if let Ok(tcp_socket) = socket.inner_tcp_mut() {
                if tcp_socket.state() != TcpSocketState::Established {
                    return Ok(());
                }

                let dst_port = tcp_socket
                    .dst_port()
                    .ok_or::<Error>("No destination port".into())?;
                let dst_addr = tcp_socket
                    .dst_ipv4_addr()
                    .ok_or::<Error>("No destination address".into())?;

                (
                    src_port,
                    dst_port,
                    dst_addr,
                    tcp_socket.seq_num(),
                    tcp_socket.next_recv_seq(),
                )
            } else {
                return Ok(());
            }
        };

        let mut packet = TcpPacket::new_with(
            src_port,
            dst_port,
            seq_num,
            ack_num,
            TcpPacket::FLAGS_ACK | TcpPacket::FLAGS_FIN,
            u16::MAX,
            0,
            Vec::new(),
            Vec::new(),
        );
        packet.calc_checksum_with_ipv4(self.my_ipv4_addr, dst_addr);

        let mut ipv4_packet = Ipv4Packet::new_with(
            0x45,
            0,
            0,
            0,
            Protocol::Tcp,
            self.my_ipv4_addr,
            dst_addr,
            Ipv4Payload::Tcp(packet),
        );
        ipv4_packet.calc_checksum();

        let target_ip = get_target_ip(self.my_ipv4_addr, dst_addr);
        let dst_mac_addr = self
            .resolve_mac_addr(target_ip)?
            .ok_or::<Error>("Failed to resolve MAC address".into())?;

        self.send_eth_payload(
            EthernetPayload::Ipv4(ipv4_packet),
            dst_mac_addr,
            EthernetType::Ipv4,
        )?;

        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;
        tcp_socket.add_seq_num(1);

        Ok(())
    }

    fn send_tcp_data(&mut self, socket_id: SocketId, data: &[u8]) -> Result<()> {
        let (src_port, dst_port, dst_addr, seq_num, ack_num) = {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;
            let src_port = socket.port();
            let tcp_socket = socket.inner_tcp_mut()?;

            if tcp_socket.state() != TcpSocketState::Established {
                return Err("Socket is not in Established state".into());
            }

            let dst_port = tcp_socket
                .dst_port()
                .ok_or::<Error>("No destination port".into())?;
            let dst_addr = tcp_socket
                .dst_ipv4_addr()
                .ok_or::<Error>("No destination address".into())?;

            (
                src_port,
                dst_port,
                dst_addr,
                tcp_socket.seq_num(),
                tcp_socket.next_recv_seq(),
            )
        };

        let mut packet = TcpPacket::new_with(
            src_port,
            dst_port,
            seq_num,
            ack_num,
            TcpPacket::FLAGS_ACK | TcpPacket::FLAGS_PSH,
            u16::MAX,
            0,
            Vec::new(),
            data.to_vec(),
        );
        packet.calc_checksum_with_ipv4(self.my_ipv4_addr, dst_addr);

        let mut ipv4_packet = Ipv4Packet::new_with(
            0x45,
            0,
            0,
            0,
            Protocol::Tcp,
            self.my_ipv4_addr,
            dst_addr,
            Ipv4Payload::Tcp(packet),
        );
        ipv4_packet.calc_checksum();

        let target_ip = get_target_ip(self.my_ipv4_addr, dst_addr);
        let dst_mac_addr = self
            .resolve_mac_addr(target_ip)?
            .ok_or::<Error>("Failed to resolve MAC address".into())?;

        self.send_eth_payload(
            EthernetPayload::Ipv4(ipv4_packet),
            dst_mac_addr,
            EthernetType::Ipv4,
        )?;

        if !data.is_empty() {
            let socket = self.socket_table.socket_mut_by_id(socket_id)?;
            let tcp_socket = socket.inner_tcp_mut()?;
            tcp_socket.add_seq_num(data.len() as u32);
        }

        Ok(())
    }

    fn recv_tcp_data(&mut self, socket_id: SocketId, buf: &mut [u8]) -> Result<usize> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;

        if !matches!(
            tcp_socket.state(),
            TcpSocketState::Established
                | TcpSocketState::FinWait1
                | TcpSocketState::FinWait2
                | TcpSocketState::CloseWait
                | TcpSocketState::TimeWait
                | TcpSocketState::LastAck
                | TcpSocketState::Closing
        ) {
            return Err("Socket is not in Established/Closing state".into());
        }

        let data = tcp_socket.get_and_reset_buf();
        if data.is_empty() {
            return Ok(0);
        }

        let len = core::cmp::min(buf.len(), data.len());
        buf[..len].copy_from_slice(&data[..len]);

        Ok(len)
    }

    fn is_tcp_established(&mut self, socket_id: SocketId) -> Result<bool> {
        let socket = self.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;
        Ok(tcp_socket.state() == TcpSocketState::Established)
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

    fn receive_tcp_packet(
        &mut self,
        packet: TcpPacket,
        remote_addr: Ipv4Addr,
    ) -> Result<Option<TcpPacket>> {
        let src_port = packet.src_port;
        let dst_port = packet.dst_port;
        let seq_num = packet.seq_num;
        let socket_mut = match self.tcp_socket_mut_by_port(dst_port, remote_addr, src_port) {
            Ok(s) => s,
            Err(e) => {
                kwarn!("net: TCP socket not found: {:?}", e);
                return Ok(None);
            }
        };

        match socket_mut.state() {
            TcpSocketState::Closed => {
                kwarn!("net: TCP received but socket is closed");
            }
            TcpSocketState::Listen => {
                if !packet.flags_syn() {
                    kwarn!("net: TCP-SYN not received");
                    return Ok(None);
                }

                let new_socket_id = self
                    .socket_table
                    .insert_new_socket(SocketType::Stream, Protocol::Tcp)?;

                let new_socket = self.socket_table.socket_mut_by_id(new_socket_id)?;
                new_socket.set_port(dst_port); // manually set port without registering to map
                let new_tcp_socket = new_socket.inner_tcp_mut()?;
                new_tcp_socket.start_passive(dst_port)?;
                new_tcp_socket.set_dst_ipv4_addr(remote_addr);
                new_tcp_socket.set_dst_port(src_port);
                let next_seq_num = new_tcp_socket.receive_syn(seq_num)?;
                let ack_num = new_tcp_socket.next_recv_seq();

                let mut options = Vec::new();
                let mss_bytes_len = 1460u16;
                options.push(0x02); // MSS
                options.push(0x04); // MSS length
                options.push((mss_bytes_len >> 8) as u8);
                options.push((mss_bytes_len & 0xff) as u8);

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
                    Vec::new(),
                );
                return Ok(Some(reply_packet));
            }
            TcpSocketState::SynSent => {
                if !packet.flags_syn() || !packet.flags_ack() {
                    kwarn!("net: TCP-SYN-ACK not received");
                    return Ok(None);
                }

                socket_mut.receive_syn_ack(seq_num)?;

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
                    Vec::new(),
                );
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

                let data = &packet.data;

                if !data.is_empty() {
                    socket_mut.receive_data(data, seq_num)?;
                    ack_needed = true;
                }

                if packet.flags_fin() {
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
                        Vec::new(),
                    );
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
        let socket_mut = self.udp_socket_mut_by_port(dst_port)?;
        socket_mut.receive(&packet.data);

        Ok(None)
    }

    fn receive_arp_packet(&mut self, packet: ArpPacket) -> Result<Option<ArpPacket>> {
        let arp_op = packet.op()?;
        let sender_ipv4_addr = packet.sender_ipv4_addr;
        let sender_mac_addr = packet.sender_eth_addr;
        let target_ipv4_addr = packet.target_ipv4_addr;

        self.arp_table.insert(
            sender_ipv4_addr,
            (
                Some(sender_mac_addr),
                device::local_apic_timer::global_uptime(),
            ),
        );

        if arp_op == ArpOperation::Request {
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

            return Ok(Some(reply_packet));
        }

        Ok(None)
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
                if !is_valid {
                    kwarn!("net: Invalid TCP checksum");
                    return Ok(None);
                }

                if let Some(mut reply_tcp_packet) =
                    self.receive_tcp_packet(tcp_packet, packet.src_addr)?
                {
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

    fn send_arp_packet(
        &mut self,
        op: ArpOperation,
        sender_eth_addr: EthernetAddress,
        sender_ipv4_addr: Ipv4Addr,
        target_eth_addr: EthernetAddress,
        target_ipv4_addr: Ipv4Addr,
    ) -> Result<()> {
        let packet = ArpPacket::new_with(
            op,
            sender_eth_addr,
            sender_ipv4_addr,
            target_eth_addr,
            target_ipv4_addr,
        );

        self.send_eth_payload(
            EthernetPayload::Arp(packet),
            target_eth_addr,
            EthernetType::Arp,
        )
    }

    fn send_udp_packet(
        &mut self,
        src_port: u16,
        dst_port: u16,
        dst_addr: Ipv4Addr,
        data: &[u8],
    ) -> Result<()> {
        let mut udp_packet = UdpPacket::new_with(src_port, dst_port, data);
        udp_packet.calc_checksum_with_ipv4(self.my_ipv4_addr, dst_addr);

        let mut ipv4_packet = Ipv4Packet::new_with(
            0x45, // version 4 + IHL 5
            0,
            0,
            0,
            Protocol::Udp,
            self.my_ipv4_addr,
            dst_addr,
            Ipv4Payload::Udp(udp_packet),
        );
        ipv4_packet.calc_checksum();

        let target_ip = get_target_ip(self.my_ipv4_addr, dst_addr);

        let dst_mac_addr = self
            .resolve_mac_addr(target_ip)?
            .ok_or::<Error>("Failed to resolve MAC address".into())?;

        self.send_eth_payload(
            EthernetPayload::Ipv4(ipv4_packet),
            dst_mac_addr,
            EthernetType::Ipv4,
        )
    }

    fn send_eth_payload(
        &mut self,
        payload: EthernetPayload,
        dst_mac_addr: EthernetAddress,
        eth_type: EthernetType,
    ) -> Result<()> {
        let payload_vec = payload.to_vec();
        let src_mac_addr = self.my_mac_addr()?;
        let eth_frame = EthernetFrame::new_with(dst_mac_addr, src_mac_addr, eth_type, &payload_vec);

        device::rtl8139::push_eth_frame_to_tx_queue(eth_frame)
    }

    fn resolve_mac_addr(&mut self, ipv4_addr: Ipv4Addr) -> Result<Option<EthernetAddress>> {
        let now = device::local_apic_timer::global_uptime();

        if let Some(entry) = self.arp_table.get_mut(&ipv4_addr) {
            match entry {
                (Some(mac), _) => return Ok(Some(*mac)),
                (None, last_req) => {
                    if now < *last_req + Duration::from_millis(1000) {
                        return Ok(None);
                    }
                    *last_req = now; // Update last request time
                }
            }
        } else {
            self.arp_table.insert(ipv4_addr, (None, now));
        }

        let eth_addr = EthernetAddress::broadcast();
        self.send_arp_packet(
            ArpOperation::Request,
            self.my_mac_addr()?,
            self.my_ipv4_addr,
            eth_addr,
            ipv4_addr,
        )?;

        Ok(None)
    }
}

pub fn set_my_mac_addr(mac_addr: EthernetAddress) -> Result<()> {
    NETWORK_MAN.try_lock()?.set_my_mac_addr(mac_addr);
    Ok(())
}

pub fn my_mac_addr() -> Result<EthernetAddress> {
    NETWORK_MAN.try_lock()?.my_mac_addr()
}

pub fn my_ipv4_addr() -> Result<Ipv4Addr> {
    let addr = NETWORK_MAN.try_lock()?.my_ipv4_addr;
    Ok(addr)
}

pub fn receive_eth_payload(payload: EthernetPayload) -> Result<Option<EthernetPayload>> {
    NETWORK_MAN.try_lock()?.receive_eth_payload(payload)
}

pub fn resolve_mac_addr(ipv4_addr: Ipv4Addr) -> Result<EthernetAddress> {
    loop {
        let eth_addr = x86_64::disabled_int(|| {
            let mut network_man = NETWORK_MAN.try_lock()?;
            let addr = network_man.resolve_mac_addr(ipv4_addr)?;
            Result::Ok(addr)
        })?;

        match eth_addr {
            Some(addr) => return Ok(addr),
            None => x86_64::stihlt(),
        }
    }
}

pub fn create_new_socket(type_: SocketType) -> Result<SocketId> {
    NETWORK_MAN.try_lock()?.create_new_socket(type_)
}

pub fn bind_socket_v4(
    socket_id: SocketId,
    bound_addr: Option<Ipv4Addr>,
    port: Option<u16>,
) -> Result<()> {
    NETWORK_MAN
        .try_lock()?
        .bind_socket_v4(socket_id, bound_addr, port)
}

pub fn sendto_udp_v4(
    socket_id: SocketId,
    dst_addr: Ipv4Addr,
    dst_port: u16,
    data: &[u8],
) -> Result<()> {
    let my_ip = my_ipv4_addr()?;
    let target_ip = get_target_ip(my_ip, dst_addr);
    resolve_mac_addr(target_ip)?;

    NETWORK_MAN
        .try_lock()?
        .sendto_udp_v4(socket_id, dst_addr, dst_port, data)
}

pub fn recvfrom_udp_v4(socket_id: SocketId, buf: &mut [u8]) -> Result<usize> {
    NETWORK_MAN.try_lock()?.recvfrom_udp_v4(socket_id, buf)
}

pub fn listen_tcp_v4(socket_id: SocketId) -> Result<()> {
    NETWORK_MAN.try_lock()?.listen_tcp_v4(socket_id)
}

pub fn accept_tcp_v4(socket_id: SocketId) -> Result<SocketId> {
    NETWORK_MAN.try_lock()?.accept_tcp_v4(socket_id)
}

pub fn connect_tcp_v4(socket_id: SocketId, dst_addr: Ipv4Addr, dst_port: u16) -> Result<()> {
    NETWORK_MAN
        .try_lock()?
        .connect_tcp_v4(socket_id, dst_addr, dst_port)
}

pub fn send_tcp_syn(socket_id: SocketId) -> Result<()> {
    // pre-resolve MAC address
    let (dst_addr, _) = {
        let mut man = NETWORK_MAN.try_lock()?;
        let socket = man.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;
        (
            tcp_socket
                .dst_ipv4_addr()
                .ok_or::<Error>("No destination address".into())?,
            tcp_socket
                .dst_port()
                .ok_or::<Error>("No destination port".into())?,
        )
    };

    let my_ip = my_ipv4_addr()?;
    let target_ip = get_target_ip(my_ip, dst_addr);
    resolve_mac_addr(target_ip)?;

    NETWORK_MAN.try_lock()?.send_tcp_syn(socket_id)
}

pub fn send_tcp_data(socket_id: SocketId, data: &[u8]) -> Result<()> {
    // pre-resolve MAC address
    let (dst_addr, _) = {
        let mut man = NETWORK_MAN.try_lock()?;
        let socket = man.socket_table.socket_mut_by_id(socket_id)?;
        let tcp_socket = socket.inner_tcp_mut()?;
        (
            tcp_socket
                .dst_ipv4_addr()
                .ok_or::<Error>("No destination address".into())?,
            tcp_socket
                .dst_port()
                .ok_or::<Error>("No destination port".into())?,
        )
    };

    let my_ip = my_ipv4_addr()?;
    let target_ip = get_target_ip(my_ip, dst_addr);
    resolve_mac_addr(target_ip)?;

    NETWORK_MAN.try_lock()?.send_tcp_data(socket_id, data)
}

pub fn recv_tcp_data(socket_id: SocketId, buf: &mut [u8]) -> Result<usize> {
    NETWORK_MAN.try_lock()?.recv_tcp_data(socket_id, buf)
}

pub fn is_tcp_established(socket_id: SocketId) -> Result<bool> {
    NETWORK_MAN.try_lock()?.is_tcp_established(socket_id)
}

pub fn close_socket(socket_id: SocketId) -> Result<()> {
    NETWORK_MAN.try_lock()?.close_socket(socket_id)
}
