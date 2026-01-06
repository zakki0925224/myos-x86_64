use crate::error::{Result, WebError};
use core::net::{Ipv4Addr, SocketAddrV4};
use libc_rs::*;

pub struct TcpStream {
    sockfd: i32,
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        unsafe { sys_close(self.sockfd) };
    }
}

impl TcpStream {
    pub fn connect(addr: &str) -> Result<Self> {
        let addr: SocketAddrV4 = addr.parse().map_err(|_| WebError::InvalidAddress)?;
        let ip = *addr.ip();
        let port = addr.port();

        let sockfd = unsafe {
            sys_socket(
                SOCKET_DOMAIN_AF_INET as i32,
                SOCKET_TYPE_SOCK_STREAM as i32,
                0,
            )
        };

        if sockfd < 0 {
            return Err(WebError::SocketCreationFailed);
        }

        let addr = sockaddr_in {
            sin_family: SOCKET_DOMAIN_AF_INET as u16,
            sin_port: port,
            sin_addr: in_addr {
                s_addr: u32::from(ip),
            },
            sin_zero: [0i8; 8],
        };

        let res = unsafe {
            sys_connect(
                sockfd,
                &addr as *const _ as *const sockaddr,
                size_of::<sockaddr_in>(),
            )
        };

        if res < 0 {
            unsafe { sys_close(sockfd) };
            return Err(WebError::ConnectionFailed);
        }

        Ok(Self { sockfd })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let n = unsafe { sys_recv(self.sockfd, buf.as_mut_ptr() as *mut _, buf.len(), 0) };

        if n < 0 {
            return Err(WebError::RecvFailed);
        }

        Ok(n as usize)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        let n = unsafe { sys_send(self.sockfd, buf.as_ptr() as *const _, buf.len(), 0) };

        if n < 0 {
            return Err(WebError::SendFailed);
        }

        Ok(n as usize)
    }
}

pub struct UdpSocket {
    sockfd: i32,
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        unsafe { sys_close(self.sockfd) };
    }
}

impl UdpSocket {
    pub fn bind(addr: &str) -> Result<Self> {
        let addr: SocketAddrV4 = addr.parse().map_err(|_| WebError::InvalidAddress)?;
        let ip = *addr.ip();
        let port = addr.port();

        let sockfd = unsafe {
            sys_socket(
                SOCKET_DOMAIN_AF_INET as i32,
                SOCKET_TYPE_SOCK_DGRAM as i32,
                SOCKET_PROTO_UDP as i32,
            )
        };

        if sockfd < 0 {
            return Err(WebError::SocketCreationFailed);
        }

        let addr = sockaddr_in {
            sin_family: SOCKET_DOMAIN_AF_INET as u16,
            sin_port: port,
            sin_addr: in_addr {
                s_addr: u32::from(ip),
            },
            sin_zero: [0i8; 8],
        };

        let res = unsafe {
            sys_bind(
                sockfd,
                &addr as *const _ as *const sockaddr,
                size_of::<sockaddr_in>(),
            )
        };

        if res < 0 {
            unsafe { sys_close(sockfd) };
            return Err(WebError::BindFailed);
        }

        Ok(Self { sockfd })
    }

    pub fn send_to(&self, buf: &[u8], addr: &str) -> Result<usize> {
        let addr: SocketAddrV4 = addr.parse().map_err(|_| WebError::InvalidAddress)?;
        let ip = *addr.ip();
        let port = addr.port();

        let addr = sockaddr_in {
            sin_family: SOCKET_DOMAIN_AF_INET as u16,
            sin_port: port,
            sin_addr: in_addr {
                s_addr: u32::from(ip),
            },
            sin_zero: [0i8; 8],
        };

        let n = unsafe {
            sys_sendto(
                self.sockfd,
                buf.as_ptr() as *const _,
                buf.len(),
                0,
                &addr as *const _ as *const sockaddr,
                size_of::<sockaddr_in>(),
            )
        };

        if n < 0 {
            return Err(WebError::SendToFailed);
        }
        Ok(n as usize)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, Ipv4Addr, u16)> {
        let mut addr = sockaddr_in {
            sin_family: 0,
            sin_port: 0,
            sin_addr: in_addr { s_addr: 0 },
            sin_zero: [0i8; 8],
        };

        let n = unsafe {
            sys_recvfrom(
                self.sockfd,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                0,
                &mut addr as *mut _ as *mut sockaddr,
                size_of::<sockaddr_in>(),
            )
        };

        if n < 0 {
            return Err(WebError::RecvFromFailed);
        }

        let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
        let port = u16::from_be(addr.sin_port);

        Ok((n as usize, ip, port))
    }
}
