use crate::{RendererSnapshot, SnapshotError};
use conduit_core::SignId;
use conduit_presentation::ManifestationFailure;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

pub const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;
const INDEX: &[u8] = include_bytes!("../assets/index.html");
const SCRIPT: &[u8] = include_bytes!("../assets/app.js");
const STYLE: &[u8] = include_bytes!("../assets/app.css");

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    Snapshot(SnapshotError),
    NonLoopbackBind,
    RequestTooLarge,
    InvalidRequest,
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Patchbay HTTP I/O error: {error}"),
            Self::Snapshot(error) => write!(f, "Patchbay snapshot error: {error}"),
            Self::NonLoopbackBind => f.write_str("Patchbay HTML binds only to IPv4 loopback"),
            Self::RequestTooLarge => f.write_str("Patchbay HTTP request exceeds its finite bound"),
            Self::InvalidRequest => f.write_str("Patchbay HTTP request is not valid UTF-8"),
        }
    }
}

impl std::error::Error for ServerError {}

#[derive(Debug)]
pub struct RendererDeliveryFailure {
    pub error: ServerError,
    pub snapshot: Box<RendererSnapshot>,
}

impl std::fmt::Display for RendererDeliveryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "renderer delivery failed: {}", self.error)
    }
}

impl std::error::Error for RendererDeliveryFailure {}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<SnapshotError> for ServerError {
    fn from(value: SnapshotError) -> Self {
        Self::Snapshot(value)
    }
}

pub struct PatchbayHtmlServer {
    listener: TcpListener,
    snapshot: RendererSnapshot,
    encoded_snapshot: Vec<u8>,
}

impl PatchbayHtmlServer {
    pub fn bind(address: SocketAddr, snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        if address.ip() != Ipv4Addr::LOCALHOST {
            return Err(ServerError::NonLoopbackBind);
        }
        let listener = TcpListener::bind(address)?;
        let mut snapshot = snapshot.clone();
        snapshot.mark_available(SignId::from("patchbay-html/document-ready"))?;
        let encoded_snapshot = snapshot.encode()?;
        Ok(Self {
            listener,
            snapshot,
            encoded_snapshot,
        })
    }

    pub fn bind_ephemeral(snapshot: &RendererSnapshot) -> Result<Self, ServerError> {
        Self::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(), snapshot)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, ServerError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn serve(mut self) -> Result<(), RendererDeliveryFailure> {
        loop {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) => return Err(self.delivery_failure(ServerError::Io(error))),
            };
            if let Err(error) = self.handle(stream) {
                return Err(self.delivery_failure(error));
            }
        }
    }

    pub fn serve_count(
        mut self,
        count: usize,
    ) -> Result<RendererSnapshot, RendererDeliveryFailure> {
        for _ in 0..count {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) => return Err(self.delivery_failure(ServerError::Io(error))),
            };
            if let Err(error) = self.handle(stream) {
                return Err(self.delivery_failure(error));
            }
        }
        if let Err(error) = self
            .snapshot
            .mark_closed(SignId::from("patchbay-html/server-closed"))
        {
            return Err(self.delivery_failure(ServerError::Snapshot(error)));
        }
        Ok(self.snapshot)
    }

    fn delivery_failure(&mut self, error: ServerError) -> RendererDeliveryFailure {
        let _ = self.snapshot.mark_failed(
            ManifestationFailure::DeliveryFailed,
            SignId::from("patchbay-html/delivery-failed"),
        );
        RendererDeliveryFailure {
            error,
            snapshot: Box::new(self.snapshot.clone()),
        }
    }

    fn handle(&self, mut stream: TcpStream) -> Result<(), ServerError> {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let request = match read_request(&mut stream) {
            Ok(request) => request,
            Err(ServerError::RequestTooLarge) => {
                return write_response(
                    &mut stream,
                    "413 Content Too Large",
                    "text/plain; charset=utf-8",
                    b"request too large",
                );
            }
            Err(ServerError::InvalidRequest) => {
                return write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid request",
                );
            }
            Err(error) => return Err(error),
        };
        let first = request.split("\r\n").next().unwrap_or_default();
        let (status, content_type, body): (&str, &str, &[u8]) = match first {
            "GET / HTTP/1.1" => ("200 OK", "text/html; charset=utf-8", INDEX),
            "GET /assets/app.js HTTP/1.1" => ("200 OK", "text/javascript; charset=utf-8", SCRIPT),
            "GET /assets/app.css HTTP/1.1" => ("200 OK", "text/css; charset=utf-8", STYLE),
            "GET /api/snapshot HTTP/1.1" => (
                "200 OK",
                "application/json; charset=utf-8",
                self.encoded_snapshot.as_slice(),
            ),
            _ if !first.starts_with("GET ") => (
                "405 Method Not Allowed",
                "text/plain; charset=utf-8",
                b"method not allowed".as_slice(),
            ),
            _ => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found".as_slice(),
            ),
        };
        write_response(&mut stream, status, content_type, body)
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), ServerError> {
    write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
    stream.write_all(body)?;
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> Result<String, ServerError> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(ServerError::RequestTooLarge);
        }
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).map_err(|_| ServerError::InvalidRequest)
}
