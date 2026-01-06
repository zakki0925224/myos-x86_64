use alloc::string::String;

#[derive(Debug, Clone, PartialEq)]
pub enum WebError {
    Failed(String),
    InvalidAddress,
    DnsResolutionFailed(String),
    SocketCreationFailed,
    ConnectionFailed,
    RecvFailed,
    SendFailed,
    BindFailed,
    SendToFailed,
    RecvFromFailed,
    InvalidReceivedResponse,
    InvalidHttpResponse(String),
}

impl From<String> for WebError {
    fn from(s: String) -> Self {
        Self::Failed(s)
    }
}

pub type Result<T> = core::result::Result<T, WebError>;
