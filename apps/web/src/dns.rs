use crate::{
    error::{Result, WebError},
    net::UdpSocket,
};
use alloc::vec::Vec;
use core::net::Ipv4Addr;
use libc_rs::sys_uptime;

pub const QEMU_DNS: &'static str = "10.0.2.3:53";
const LOCALHOST_ADDR: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
const DNS_TIMEOUT_MS: u64 = 5000;

pub struct DnsClient {
    dns_server: &'static str,
}

impl DnsClient {
    pub fn new(dns_server: &'static str) -> Self {
        Self { dns_server }
    }

    pub fn resolve_all(&self, domain: &str) -> Result<Vec<Ipv4Addr>> {
        if domain == "localhost" {
            return Ok(vec![LOCALHOST_ADDR]);
        }

        // Skip DNS for raw IPv4 addresses
        if let Ok(ip) = domain.parse::<Ipv4Addr>() {
            return Ok(vec![ip]);
        }

        let socket = UdpSocket::bind("0.0.0.0:0")?;

        // RFC 1035
        let mut query = Vec::new();

        // 4.1.1. Header section format
        query.extend_from_slice(&0x1234u16.to_be_bytes()); // ID
        query.extend_from_slice(&0x0100u16.to_be_bytes()); // SQ+RD
        query.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
        query.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
        query.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
        query.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT

        // 4.1.2. Question section format
        for label in domain.split(".") {
            if label.is_empty() {
                continue;
            }

            query.push(label.len() as u8);
            query.extend_from_slice(label.as_bytes());
        }
        query.push(0);

        query.extend_from_slice(&1u16.to_be_bytes()); // QTYPE: A
        query.extend_from_slice(&1u16.to_be_bytes()); // QCLASS: IN

        // send
        socket.send_to(&query, self.dns_server)?;

        // receive with real-time timeout
        let mut buf = [0u8; 1500];
        let mut n = 0;
        let start = unsafe { sys_uptime() };

        loop {
            let elapsed = unsafe { sys_uptime() } - start;
            if elapsed > DNS_TIMEOUT_MS {
                break;
            }

            let (res, _, _) = socket.recv_from(&mut buf)?;
            if res > 0 {
                n = res;
                break;
            }
            unsafe { core::arch::asm!("pause") };
        }

        if n == 0 {
            return Err(WebError::DnsResolutionFailed(
                "Timed out waiting for DNS response".into(),
            ));
        }

        let buf = &buf[..n];

        // parse response
        if buf.len() < 12 {
            return Err(WebError::DnsResolutionFailed("Short DNS response".into()));
        }

        let id = u16::from_be_bytes([buf[0], buf[1]]);
        let ancount = u16::from_be_bytes([buf[6], buf[7]]);
        if id != 0x1234 || ancount == 0 {
            return Err(WebError::DnsResolutionFailed(
                "Invalid DNS ID or empty response".into(),
            ));
        }

        let mut pos = 12;
        pos = self.skip_name(buf, pos)?;
        pos += 4;

        let mut addrs = Vec::new();

        for _ in 0..ancount {
            pos = self.skip_name(buf, pos)?;
            let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
            let rdlen = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]);
            pos += 10;

            if rtype == 1 && rdlen == 4 {
                // Type A (IPv4)
                addrs.push(Ipv4Addr::new(
                    buf[pos],
                    buf[pos + 1],
                    buf[pos + 2],
                    buf[pos + 3],
                ));
            }
            pos += rdlen as usize;
        }

        if addrs.is_empty() {
            return Err(WebError::DnsResolutionFailed("No DNS records found".into()));
        }

        Ok(addrs)
    }

    fn skip_name(&self, buf: &[u8], mut pos: usize) -> Result<usize> {
        while pos < buf.len() {
            let b = buf[pos];

            if b == 0 {
                return Ok(pos + 1);
            }

            if (b & 0xc0) == 0xc0 {
                return Ok(pos + 2);
            }

            pos += b as usize + 1;
        }

        Err(WebError::DnsResolutionFailed(
            "Buffer overflow decoding name".into(),
        ))
    }
}
