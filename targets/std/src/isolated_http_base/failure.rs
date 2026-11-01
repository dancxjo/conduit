//! Finite transport diagnostics, without endpoint, payload or worker text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpExchangeRefusal {
    Authorization,
    Capacity,
    TransportTimeout,
    TransportIo,
    ConnectionRefused,
    ProviderSpawn,
    ProviderProtocol,
    ProviderRefused,
    MalformedResponse,
}
impl From<HttpExchangeRefusal> for String {
    fn from(value: HttpExchangeRefusal) -> Self {
        format!("isolated HTTP {value:?}")
    }
}
impl HttpExchangeRefusal {
    pub(super) fn io(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => Self::TransportTimeout,
            std::io::ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            _ => Self::TransportIo,
        }
    }
    pub(super) fn provider(reason: &str) -> Self {
        match reason {
            "transport:timeout" => Self::TransportTimeout,
            "transport:io" => Self::TransportIo,
            "response:malformed" => Self::MalformedResponse,
            "request:malformed-or-oversized" => Self::Capacity,
            "endpoint:wrong-authority" => Self::Authorization,
            _ if reason.starts_with("capability:") => Self::Authorization,
            _ => Self::ProviderRefused,
        }
    }
}
