use crate::{
    error::{Error, Result},
    net::{ip::Protocol, tcp::TcpSocket, udp::UdpSocket},
};
use alloc::collections::btree_map::BTreeMap;
use core::{
    fmt,
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
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn new_val(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(Error::Failed("Invalid socket file descriptor number"));
        }

        Ok(Self(value as usize))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum SocketInner {
    Udp(UdpSocket),
    Tcp(TcpSocket),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SocketType {
    Stream, // TCP
    Dgram,  // UDP
}

#[derive(Debug)]
pub struct Socket {
    port: u16,
    inner: SocketInner,
    type_: SocketType,
}

impl Socket {
    pub fn inner_udp_mut(&mut self) -> Result<&mut UdpSocket> {
        if self.type_ != SocketType::Dgram {
            return Err(Error::Failed("Invalid socket type"));
        }

        match &mut self.inner {
            SocketInner::Udp(socket) => Ok(socket),
            _ => Err(Error::Failed("Invalid socket type")),
        }
    }

    pub fn inner_tcp_mut(&mut self) -> Result<&mut TcpSocket> {
        if self.type_ != SocketType::Stream {
            return Err(Error::Failed("Invalid socket type"));
        }

        match &mut self.inner {
            SocketInner::Tcp(socket) => Ok(socket),
            _ => Err(Error::Failed("Invalid socket type")),
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

    pub fn socket_mut_by_id(&mut self, id: SocketId) -> Result<&mut Socket> {
        self.table
            .get_mut(&id)
            .ok_or(Error::Failed("Invalid socket ID"))
    }

    pub fn socket_mut_by_port_and_type(
        &mut self,
        port: u16,
        type_: SocketType,
    ) -> Result<&mut Socket> {
        let socket_id = match type_ {
            SocketType::Stream => self.tcp_port_socket_id_map.get(&port),
            SocketType::Dgram => self.udp_port_socket_id_map.get(&port),
        }
        .ok_or(Error::Failed("Port is not used"))?;

        self.socket_mut_by_id(*socket_id)
    }

    pub fn insert_new_socket(&mut self, type_: SocketType, protocol: Protocol) -> Result<SocketId> {
        let inner = match type_ {
            SocketType::Stream => {
                if protocol != Protocol::Tcp {
                    return Err(Error::Failed("Invalid protocol"));
                }

                SocketInner::Tcp(TcpSocket::new())
            }
            SocketType::Dgram => {
                if protocol != Protocol::Udp {
                    return Err(Error::Failed("Invalid protocol"));
                }

                SocketInner::Udp(UdpSocket::new())
            }
        };

        let id = SocketId::new();
        let socket = Socket {
            port: 0, // unbound
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
                return Err(Error::Failed("All ephemeral ports are already in use"));
            }
        } else {
            if self.tcp_port_socket_id_map.contains_key(&port) {
                return Err(Error::Failed("Port is already in use (TCP)"));
            }
            if self.udp_port_socket_id_map.contains_key(&port) {
                return Err(Error::Failed("Port is already in use (UDP)"));
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
}
