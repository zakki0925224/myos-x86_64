use crate::{
    dns::*,
    error::{Result, WebError},
    net::TcpStream,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::net::{IpAddr, SocketAddr};
use libc_rs::println;

#[derive(Debug, Clone)]
pub struct Header {
    name: String,
    value: String,
}

impl Header {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    version: String,
    status_code: u32,
    reason: String,
    headers: Vec<Header>,
    body: String,
}

impl HttpResponse {
    pub fn new(raw_response: String) -> Result<Self> {
        let preprocessed_response = raw_response
            .trim_start()
            .replace("\r\n", "\n")
            .replace("\n\t", "\n");

        let (status_line, remaining) = match preprocessed_response.split_once("\n") {
            Some((s, r)) => (s, r),
            None => return Err(WebError::InvalidHttpResponse(preprocessed_response)),
        };

        let (headers, body) = match remaining.split_once("\n\n") {
            Some((h, b)) => {
                let mut headers = Vec::new();
                for header in h.split("\n") {
                    let splitted_header: Vec<&str> = header.splitn(2, ":").collect();
                    headers.push(Header::new(
                        String::from(splitted_header[0].trim()),
                        String::from(splitted_header[1].trim()),
                    ));
                }

                (headers, b)
            }
            None => (Vec::new(), remaining),
        };

        let statuses: Vec<&str> = status_line.split_whitespace().collect();

        Ok(Self {
            version: statuses[0].to_string(),
            status_code: statuses[1].parse().unwrap_or(404),
            reason: statuses[2].to_string(),
            headers,
            body: body.to_string(),
        })
    }
}

pub struct HttpClient;

impl HttpClient {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, host: String, port: u16, path: String) -> Result<HttpResponse> {
        let dns_client = DnsClient::new(QEMU_DNS);
        let ip = dns_client.resolve(&host)?;
        println!("{:?} -> {:?}", host, ip);
        let socket_addr = SocketAddr::new(IpAddr::V4(ip), port);

        let stream = TcpStream::connect(&socket_addr.to_string())?;

        let mut request = String::from("GET ");
        request.push_str(&path);
        request.push_str(" HTTP/1.1\r\n");
        request.push_str("Host: ");
        request.push_str(&host);
        request.push_str("\r\n");
        request.push_str("Accept: text/html\r\n");
        request.push_str("Connection: close\r\n");
        request.push_str("\r\n");

        let _ = stream.write(request.as_bytes())?;

        let mut received = Vec::new();
        loop {
            let mut buf = [0u8; 4096];
            let bytes_read = stream.read(&mut buf)?;

            if bytes_read == 0 {
                break;
            }

            received.extend_from_slice(&buf[..bytes_read]);
        }

        match core::str::from_utf8(&received) {
            Ok(res) => HttpResponse::new(res.to_string()),
            Err(_) => Err(WebError::InvalidReceivedResponse),
        }
    }
}
