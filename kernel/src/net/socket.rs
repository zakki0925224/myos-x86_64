use crate::{
    error::{Error, Result},
    net::{
        ip::Protocol,
        tcp::{TcpSocket, TcpSocketState},
        udp::UdpSocket,
    },
};
use alloc::collections::btree_map::BTreeMap;
use core::{
    fmt,
    net::Ipv4Addr,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SocketId(usize);

impl fmt::Display for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SocketId {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(4096);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn new_val(value: i32) -> Result<Self> {
        if value < 0 {
            return Err("Invalid socket file descriptor number".into());
        }

        Ok(Self(value as usize))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum SocketInner {
    Tcp(TcpSocket),
    Udp(UdpSocket),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SocketType {
    Stream, // TCP
    Dgram,  // UDP
}

#[derive(Debug)]
pub struct Socket {
    port: u16,
    pub addr: Option<Ipv4Addr>,
    inner: SocketInner,
    type_: SocketType,
}

impl Socket {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn type_(&self) -> SocketType {
        self.type_
    }

    pub fn inner_udp_mut(&mut self) -> Result<&mut UdpSocket> {
        if self.type_ != SocketType::Dgram {
            return Err("Invalid socket type".into());
        }

        match &mut self.inner {
            SocketInner::Udp(socket) => Ok(socket),
            _ => Err("Invalid socket type".into()),
        }
    }

    pub fn inner_tcp_mut(&mut self) -> Result<&mut TcpSocket> {
        if self.type_ != SocketType::Stream {
            return Err("Invalid socket type".into());
        }

        match &mut self.inner {
            SocketInner::Tcp(socket) => Ok(socket),
            _ => Err("Invalid socket type".into()),
        }
    }
}

#[derive(Debug)]
pub struct SocketTable {
    table: BTreeMap<SocketId, Socket>,
    udp_port_socket_id_map: BTreeMap<u16, SocketId>,
    tcp_port_socket_id_map: BTreeMap<u16, SocketId>,
}

impl SocketTable {
    const PORT_EPHEMERAL_START: u16 = 49152;
    const PORT_EPHEMERAL_END: u16 = 65535;

    pub const fn new() -> Self {
        Self {
            table: BTreeMap::new(),
            udp_port_socket_id_map: BTreeMap::new(),
            tcp_port_socket_id_map: BTreeMap::new(),
        }
    }

    pub fn socket_by_id(&self, id: SocketId) -> Result<&Socket> {
        self.table.get(&id).ok_or("Invalid socket ID".into())
    }

    pub fn socket_mut_by_id(&mut self, id: SocketId) -> Result<&mut Socket> {
        self.table.get_mut(&id).ok_or("Invalid socket ID".into())
    }

    pub fn remove_socket(&mut self, id: SocketId) -> Result<()> {
        let socket = self
            .table
            .remove(&id)
            .ok_or::<Error>("Invalid socket ID".into())?;

        let port = socket.port();
        if port != 0 {
            match socket.type_() {
                SocketType::Stream => {
                    self.tcp_port_socket_id_map.remove(&port);
                }
                SocketType::Dgram => {
                    self.udp_port_socket_id_map.remove(&port);
                }
            }
        }
        Ok(())
    }

    pub fn socket_id_by_port_and_type(&self, port: u16, type_: SocketType) -> Result<SocketId> {
        let socket_id = match type_ {
            SocketType::Stream => self.tcp_port_socket_id_map.get(&port),
            SocketType::Dgram => self.udp_port_socket_id_map.get(&port),
        }
        .ok_or::<Error>("Port is not used".into())?;

        Ok(*socket_id)
    }

    pub fn insert_new_socket(&mut self, type_: SocketType, protocol: Protocol) -> Result<SocketId> {
        let inner = match type_ {
            SocketType::Stream => {
                if protocol != Protocol::Tcp {
                    return Err("Invalid protocol".into());
                }

                SocketInner::Tcp(TcpSocket::new())
            }
            SocketType::Dgram => {
                if protocol != Protocol::Udp {
                    return Err("Invalid protocol".into());
                }

                SocketInner::Udp(UdpSocket::new())
            }
        };

        let id = SocketId::new();
        let socket = Socket {
            port: 0,    // unbound
            addr: None, // unbound
            inner,
            type_,
        };
        self.table.insert(id, socket);

        Ok(id)
    }

    pub fn bind_port(&mut self, socket_id: SocketId, port: Option<u16>) -> Result<()> {
        // validate port
        let mut port = port.unwrap_or(0);
        // select port automatically
        if port == 0 {
            for p in Self::PORT_EPHEMERAL_START..=Self::PORT_EPHEMERAL_END {
                if !self.tcp_port_socket_id_map.contains_key(&p)
                    && !self.udp_port_socket_id_map.contains_key(&p)
                {
                    port = p;
                    break;
                }
            }

            if port == 0 {
                return Err("All ephemeral ports are already in use".into());
            }
        } else {
            if self.tcp_port_socket_id_map.contains_key(&port) {
                return Err("Port is already in use (TCP)".into());
            }
            if self.udp_port_socket_id_map.contains_key(&port) {
                return Err("Port is already in use (UDP)".into());
            }
        }

        // get target socket
        let socket = self.socket_mut_by_id(socket_id)?;

        // update port and port mapping
        socket.port = port;
        match socket.type_ {
            SocketType::Stream => {
                self.tcp_port_socket_id_map.insert(port, socket_id);
            }
            SocketType::Dgram => {
                self.udp_port_socket_id_map.insert(port, socket_id);
            }
        }

        Ok(())
    }

    pub fn find_tcp_established_socket(&self, server_port: u16) -> Option<SocketId> {
        for (socket_id, socket) in self.table.iter() {
            if socket.type_() != SocketType::Stream {
                continue;
            }

            if socket.port() != server_port {
                continue;
            }

            let tcp_socket = match &socket.inner {
                SocketInner::Tcp(s) => s,
                _ => continue,
            };

            if tcp_socket.state() == TcpSocketState::Established {
                return Some(*socket_id);
            }
        }

        None
    }

    pub fn find_tcp_socket_by_port_and_addr(
        &self,
        local_port: u16,
        remote_addr: Ipv4Addr,
        remote_port: u16,
    ) -> Option<SocketId> {
        for (socket_id, socket) in self.table.iter() {
            if socket.type_() != SocketType::Stream {
                continue;
            }

            if socket.port() != local_port {
                continue;
            }

            let tcp_socket = match &socket.inner {
                SocketInner::Tcp(s) => s,
                _ => continue,
            };

            if let (Some(addr), Some(port)) = (tcp_socket.dst_ipv4_addr(), tcp_socket.dst_port()) {
                if addr == remote_addr && port == remote_port {
                    return Some(*socket_id);
                }
            }
        }

        None
    }
}
