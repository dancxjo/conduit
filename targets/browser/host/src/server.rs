//! Finite loopback delivery for one standalone browser Host page.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;

mod application;
mod target_assets;

const INDEX: &[u8] = include_bytes!("../assets/index.html");
const BOOTSTRAP: &[u8] = include_bytes!("../assets/host.mjs");
const HOST_BOOTSTRAP: &[u8] = include_bytes!("../assets/browser-host-bootstrap.mjs");
const HOST_MEMBERSHIP: &[u8] = include_bytes!("../assets/browser-host-membership.mjs");
const HOST_IDENTITY: &[u8] = include_bytes!("../assets/browser-host-identity.mjs");
const APPLICATION_PRESENTATION: &[u8] = include_bytes!("../assets/application-presentation.mjs");
const APPLICATION_THEME_MODULE: &[u8] = include_bytes!("../assets/application-theme.mjs");
const BROWSER_HOST_OPERATIONS: &[u8] = include_bytes!("../assets/browser-host-operations.mjs");
const APPLICATION_THEME: &[u8] = include_bytes!("../assets/application-theme.css");
const MEDIA_HOST: &[u8] = include_bytes!("../assets/media-host.mjs");
const DEVICE_BASE: &[u8] = include_bytes!("../assets/device-base.mjs");
const USB_DEVICE_BASE: &[u8] = include_bytes!("../assets/usb-device-base.mjs");
const INITIAL_BODY_FORMS: &str = concat!(
    include_str!("../../../../forms/morse-network/main.conduit"),
    "\n",
    include_str!("../../../../forms/memory-lantern/main.conduit"),
    "\n",
    include_str!("../../../../forms/desk-telegraph/main.conduit"),
    "\n",
    include_str!("../../../../forms/button-across-room/main.conduit"),
);
const MAX_RUNTIME_BYTES: usize = 8 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 4096;
const MAX_REQUESTS: usize = 1024;

#[derive(Debug)]
pub struct BrowserHostServer {
    listener: TcpListener,
    runtime: Option<Vec<u8>>,
    application: Option<application::ApplicationDirectory>,
}

impl BrowserHostServer {
    pub fn bind(runtime_path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(runtime_path).map_err(|error| {
            format!(
                "browser Host runtime {} is unavailable ({error})",
                runtime_path.display()
            )
        })?;
        if !metadata.is_file() || metadata.len() > MAX_RUNTIME_BYTES as u64 {
            return Err(format!(
                "browser Host runtime {} exceeds the admitted {MAX_RUNTIME_BYTES}-byte artifact bound",
                runtime_path.display()
            ));
        }
        let runtime = std::fs::read(runtime_path)
            .map_err(|error| format!("cannot read browser Host runtime: {error}"))?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("cannot bind ephemeral browser Host entrance: {error}"))?;
        Ok(Self {
            listener,
            runtime: Some(runtime),
            application: None,
        })
    }

    pub fn bind_application(root: &Path, mount: &str) -> Result<Self, String> {
        let application = application::ApplicationDirectory::admit(root, mount)?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("cannot bind ephemeral browser Host entrance: {error}"))?;
        Ok(Self {
            listener,
            runtime: None,
            application: Some(application),
        })
    }

    pub fn url(&self) -> Result<String, String> {
        let mount = self
            .application
            .as_ref()
            .map_or("/", application::ApplicationDirectory::mount);
        self.listener
            .local_addr()
            .map(|address| format!("http://{address}{mount}"))
            .map_err(|error| format!("cannot resolve browser Host entrance: {error}"))
    }

    #[cfg(test)]
    pub fn local_addr(&self) -> Result<std::net::SocketAddr, String> {
        self.listener
            .local_addr()
            .map_err(|error| format!("cannot resolve browser Host entrance: {error}"))
    }

    pub fn serve(self) -> Result<(), String> {
        for request_index in 0..MAX_REQUESTS {
            let (mut stream, _) = self
                .listener
                .accept()
                .map_err(|error| format!("browser Host entrance accept failed: {error}"))?;
            self.respond(&mut stream)?;
            if request_index + 1 == MAX_REQUESTS {
                return Err(format!(
                    "browser Host entrance exhausted its admitted {MAX_REQUESTS}-request lifetime"
                ));
            }
        }
        unreachable!()
    }

    fn respond(&self, stream: &mut TcpStream) -> Result<(), String> {
        let mut request = [0_u8; MAX_REQUEST_BYTES];
        let mut length = 0;
        while length < request.len() && !request[..length].ends_with(b"\r\n\r\n") {
            let read = stream
                .read(&mut request[length..])
                .map_err(|error| format!("browser Host request read failed: {error}"))?;
            if read == 0 {
                break;
            }
            length += read;
        }
        if length == request.len() && !request.ends_with(b"\r\n\r\n") {
            return Err(format!(
                "browser Host request exceeded the admitted {MAX_REQUEST_BYTES}-byte bound"
            ));
        }
        let request = std::str::from_utf8(&request[..length])
            .map_err(|_| "browser Host request was not UTF-8".to_owned())?;
        let request_line = request.lines().next();
        if request_line == Some("GET /forms/initial-body.conduit HTTP/1.1") {
            return self.write_response(
                stream,
                "200 OK",
                "text/plain; charset=utf-8",
                INITIAL_BODY_FORMS.as_bytes(),
            );
        }
        if let Some(application) = &self.application {
            return match application.response(request_line)? {
                Some(response) => {
                    self.write_response(stream, "200 OK", response.content_type, &response.body)
                }
                None => self.write_response(
                    stream,
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"not found",
                ),
            };
        }
        if let Some((content_type, body)) = target_assets::response(request_line) {
            return self.write_response(stream, "200 OK", content_type, body);
        }
        let (status, content_type, body): (&str, &str, &[u8]) = match request_line {
            Some("GET / HTTP/1.1") => ("200 OK", "text/html; charset=utf-8", INDEX),
            Some("GET /host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", BOOTSTRAP)
            }
            Some("GET /browser-host-bootstrap.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_BOOTSTRAP)
            }
            Some("GET /browser-host-membership.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_MEMBERSHIP)
            }
            Some("GET /browser-host-identity.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_IDENTITY)
            }
            Some("GET /assets/application-presentation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_PRESENTATION,
            ),
            Some("GET /assets/application-theme.css HTTP/1.1") => {
                ("200 OK", "text/css; charset=utf-8", APPLICATION_THEME)
            }
            Some("GET /assets/application-theme.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_THEME_MODULE,
            ),
            Some("GET /assets/browser-host-operations.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BROWSER_HOST_OPERATIONS,
            ),
            Some("GET /media-host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", MEDIA_HOST)
            }
            Some("GET /device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", DEVICE_BASE)
            }
            Some("GET /usb-device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", USB_DEVICE_BASE)
            }
            Some("GET /runtime.wasm HTTP/1.1") => (
                "200 OK",
                "application/wasm",
                self.runtime.as_deref().expect("bare Host has a runtime"),
            ),
            _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found"),
        };
        self.write_response(stream, status, content_type, body)
    }

    fn write_response(
        &self,
        stream: &mut TcpStream,
        status: &str,
        content_type: &str,
        body: &[u8],
    ) -> Result<(), String> {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .and_then(|()| stream.write_all(body))
        .map_err(|error| format!("browser Host response failed: {error}"))
    }
}

#[cfg(test)]
mod tests;
