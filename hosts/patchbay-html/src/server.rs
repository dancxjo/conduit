use crate::{RendererSnapshot, SnapshotError};
use conduit_core::SignId;
use conduit_presentation::ManifestationFailure;
use patchbay_model::{PatchbayInteraction, PHOSPHOR_THEME};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

mod browser_membership;
mod front_door;
mod interaction;
mod parts;
mod theme;
mod transition;

use theme::render_theme_css;

pub const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;
pub const MAX_THEME_CSS_BYTES: usize = 2 * 1024;
const INDEX: &[u8] = include_bytes!("../assets/index.html");
const SCRIPT: &[u8] = include_bytes!("../assets/app.js");
const FLOW_SCRIPT: &[u8] = include_bytes!("../assets/flow.js");
const MEMBERSHIP_SCRIPT: &[u8] = include_bytes!("../assets/browser-membership.js");
const STYLE: &[u8] = include_bytes!("../assets/app.css");
const REACT: &[u8] = include_bytes!("../assets/react.min.js");
const REACT_DOM: &[u8] = include_bytes!("../assets/react-dom.min.js");
const REACT_FLOW: &[u8] = include_bytes!("../assets/react-flow.min.js");
const REACT_FLOW_STYLE: &[u8] = include_bytes!("../assets/react-flow.css");
const MAX_BROWSER_WASM_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    Snapshot(SnapshotError),
    NonLoopbackBind,
    ThemeCssTooLarge,
    RequestTooLarge,
    InvalidRequest,
    Interaction(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Patchbay HTTP I/O error: {error}"),
            Self::Snapshot(error) => write!(f, "Patchbay snapshot error: {error}"),
            Self::NonLoopbackBind => f.write_str("Patchbay HTML binds only to IPv4 loopback"),
            Self::ThemeCssTooLarge => {
                f.write_str("Patchbay theme CSS exceeds its finite encoded bound")
            }
            Self::RequestTooLarge => f.write_str("Patchbay HTTP request exceeds its finite bound"),
            Self::InvalidRequest => f.write_str("Patchbay HTTP request is not valid UTF-8"),
            Self::Interaction(error) => write!(f, "Patchbay interaction error: {error}"),
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
    theme_css: Vec<u8>,
    interaction: PatchbayInteraction,
    front_door: Option<std::sync::Arc<std::sync::Mutex<patchbay_model::LocalFrontDoor>>>,
    zero_body_front_door:
        Option<std::sync::Arc<std::sync::Mutex<patchbay_model::ZeroBodyFrontDoor>>>,
    body_admission: Option<Vec<u8>>,
    browser_wasm: Option<Vec<u8>>,
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
        let theme_css = render_theme_css(&PHOSPHOR_THEME);
        if theme_css.len() > MAX_THEME_CSS_BYTES {
            return Err(ServerError::ThemeCssTooLarge);
        }
        Ok(Self {
            listener,
            snapshot,
            encoded_snapshot,
            theme_css,
            interaction: PatchbayInteraction::new(
                conduit_core::HostId::from("patchbay-html/interaction-host"),
                conduit_core::BootId::from("patchbay-html/interaction-boot"),
            ),
            front_door: None,
            zero_body_front_door: None,
            body_admission: None,
            browser_wasm: None,
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

    fn handle(&mut self, mut stream: TcpStream) -> Result<(), ServerError> {
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
        let first = request.head.split("\r\n").next().unwrap_or_default();
        if first == "POST /api/parts-interaction HTTP/1.1" {
            let body = match self.apply_parts_interaction(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid Parts interaction request",
                    );
                }
                Err(error) => return Err(error),
            };
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
        if first == "POST /api/front-door-transition HTTP/1.1" {
            let body = self.apply_front_door_transition(&request.body)?;
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
        if first == "POST /api/interaction HTTP/1.1" {
            let body = match self.apply_interaction(&request.body) {
                Ok(body) => body,
                Err(ServerError::InvalidRequest) => {
                    return write_response(
                        &mut stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        b"invalid interaction request",
                    );
                }
                Err(error) => return Err(error),
            };
            return write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            );
        }
        if first == "GET /api/snapshot HTTP/1.1"
            && (self.front_door.is_some() || self.zero_body_front_door.is_some())
        {
            self.refresh_front_door()?;
        }
        let (status, content_type, body): (&str, &str, &[u8]) = match first {
            "GET / HTTP/1.1" => ("200 OK", "text/html; charset=utf-8", INDEX),
            "GET /assets/app.js HTTP/1.1" => ("200 OK", "text/javascript; charset=utf-8", SCRIPT),
            "GET /assets/flow.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", FLOW_SCRIPT)
            }
            "GET /assets/react.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT)
            }
            "GET /assets/react-dom.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT_DOM)
            }
            "GET /assets/react-flow.min.js HTTP/1.1" => {
                ("200 OK", "text/javascript; charset=utf-8", REACT_FLOW)
            }
            "GET /assets/browser-membership.js HTTP/1.1" => (
                "200 OK",
                "text/javascript; charset=utf-8",
                MEMBERSHIP_SCRIPT,
            ),
            "GET /assets/conduit-browser-runtime.wasm HTTP/1.1" => {
                self.browser_wasm.as_deref().map_or(
                    (
                        "404 Not Found",
                        "text/plain; charset=utf-8",
                        b"browser Host runtime unavailable".as_slice(),
                    ),
                    |body| ("200 OK", "application/wasm", body),
                )
            }
            "GET /api/body-admission HTTP/1.1" => self.body_admission.as_deref().map_or(
                (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"browser membership unavailable".as_slice(),
                ),
                |body| ("200 OK", "application/json; charset=utf-8", body),
            ),
            "GET /assets/app.css HTTP/1.1" => ("200 OK", "text/css; charset=utf-8", STYLE),
            "GET /assets/react-flow.css HTTP/1.1" => {
                ("200 OK", "text/css; charset=utf-8", REACT_FLOW_STYLE)
            }
            "GET /assets/theme.css HTTP/1.1" => (
                "200 OK",
                "text/css; charset=utf-8",
                self.theme_css.as_slice(),
            ),
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
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self' ws://127.0.0.1:*; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
    stream.write_all(body)?;
    Ok(())
}

struct HttpRequest {
    head: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ServerError> {
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
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let head_end = header_end + 4;
            let head = std::str::from_utf8(&bytes[..head_end])
                .map_err(|_| ServerError::InvalidRequest)?
                .to_owned();
            let content_length = head
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| ServerError::InvalidRequest)?
                .unwrap_or(0);
            if head_end
                .checked_add(content_length)
                .is_none_or(|total| total > MAX_HTTP_REQUEST_BYTES)
            {
                return Err(ServerError::RequestTooLarge);
            }
            while bytes.len() < head_end + content_length {
                let count = stream.read(&mut chunk)?;
                if count == 0 {
                    return Err(ServerError::InvalidRequest);
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            return Ok(HttpRequest {
                head,
                body: bytes[head_end..head_end + content_length].to_vec(),
            });
        }
    }
    Err(ServerError::InvalidRequest)
}
