//! Finite loopback delivery for one standalone browser Host page.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;

mod book_assets;
mod creche_assets;
mod existing_computer_assets;
mod surface;
use surface::ProductDocument;
pub use surface::ProductSurface;

const INDEX: &[u8] = include_bytes!("../assets/index.html");
const BOOTSTRAP: &[u8] = include_bytes!("../assets/host.mjs");
const HOST_BOOTSTRAP: &[u8] = include_bytes!("../assets/browser-host-bootstrap.mjs");
const HOST_MEMBERSHIP: &[u8] = include_bytes!("../assets/browser-host-membership.mjs");
const APPLICATION_LOADER: &[u8] = include_bytes!("../assets/browser-application-loader.mjs");
const APPLICATION_STORAGE: &[u8] = include_bytes!("../assets/browser-application-storage.mjs");
const APPLICATION_PRESENTATION: &[u8] = include_bytes!("../assets/application-presentation.mjs");
const MEDIA_HOST: &[u8] = include_bytes!("../assets/media-host.mjs");
const DEVICE_BASE: &[u8] = include_bytes!("../assets/device-base.mjs");
const USB_DEVICE_BASE: &[u8] = include_bytes!("../assets/usb-device-base.mjs");
const MAX_RUNTIME_BYTES: usize = 5 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 4096;
const MAX_REQUESTS: usize = 1024;

#[derive(Debug)]
pub struct BrowserHostServer {
    listener: TcpListener,
    runtime: Vec<u8>,
    book_application: Vec<u8>,
    creche_application: Vec<u8>,
    surface: ProductSurface,
}

impl BrowserHostServer {
    pub fn bind(runtime_path: &Path, surface: ProductSurface) -> Result<Self, String> {
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
        let book_application = book_assets::build_manifest(&runtime)?;
        let creche_application = creche_assets::build_manifest(&runtime)?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("cannot bind ephemeral browser Host entrance: {error}"))?;
        Ok(Self {
            listener,
            runtime,
            book_application,
            creche_application,
            surface,
        })
    }

    pub fn url(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|address| format!("http://{address}/"))
            .map_err(|error| format!("cannot resolve browser Host entrance: {error}"))
    }

    pub fn book_url(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|address| format!("http://{address}/book/"))
            .map_err(|error| format!("cannot resolve executable-book entrance: {error}"))
    }

    pub fn creche_url(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|address| format!("http://{address}/creche/"))
            .map_err(|error| format!("cannot resolve Crèche entrance: {error}"))
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
        if !self.surface.permits(request_line) {
            return self.write_response(
                stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found",
            );
        }
        if let Some(document) = self.surface.document(request_line) {
            let body = match document {
                ProductDocument::Book => book_assets::DOCUMENT,
                ProductDocument::Creche => creche_assets::DOCUMENT,
            };
            return self.write_response(stream, "200 OK", "text/html; charset=utf-8", body);
        }
        if let Some((content_type, body)) = book_assets::response(request_line) {
            return self.write_response(stream, "200 OK", content_type, body);
        }
        if let Some((content_type, body)) = existing_computer_assets::response(request_line) {
            return self.write_response(stream, "200 OK", content_type, body);
        }
        if request_line == Some("GET /book/book.application.json HTTP/1.1") {
            return self.write_response(
                stream,
                "200 OK",
                "application/json; charset=utf-8",
                &self.book_application,
            );
        }
        if request_line == Some("GET /creche/creche.application.json HTTP/1.1") {
            return self.write_response(
                stream,
                "200 OK",
                "application/json; charset=utf-8",
                &self.creche_application,
            );
        }
        let (status, content_type, body): (&str, &str, &[u8]) = match request_line {
            Some("GET / HTTP/1.1") => ("200 OK", "text/html; charset=utf-8", INDEX),
            Some("GET /host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", BOOTSTRAP)
            }
            Some("GET /browser-host-bootstrap.mjs HTTP/1.1")
            | Some("GET /book/browser-host-bootstrap.mjs HTTP/1.1")
            | Some("GET /creche/browser-host-bootstrap.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_BOOTSTRAP)
            }
            Some("GET /browser-host-membership.mjs HTTP/1.1")
            | Some("GET /book/browser-host-membership.mjs HTTP/1.1")
            | Some("GET /creche/browser-host-membership.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_MEMBERSHIP)
            }
            Some("GET /book/browser-application-loader.mjs HTTP/1.1")
            | Some("GET /creche/browser-application-loader.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_LOADER,
            ),
            Some("GET /book/browser-application-storage.mjs HTTP/1.1")
            | Some("GET /creche/browser-application-storage.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_STORAGE,
            ),
            Some("GET /book/application-presentation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_PRESENTATION,
            ),
            Some("GET /assets/application-presentation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_PRESENTATION,
            ),
            Some("GET /media-host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", MEDIA_HOST)
            }
            Some("GET /device-base.mjs HTTP/1.1")
            | Some("GET /creche/device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", DEVICE_BASE)
            }
            Some("GET /usb-device-base.mjs HTTP/1.1")
            | Some("GET /creche/usb-device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", USB_DEVICE_BASE)
            }
            Some("GET /targets/rp2040/browser-deployment/index.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/index.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_DEPLOYMENT,
            ),
            Some("GET /targets/rp2040/browser-deployment/deployment.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/deployment.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_DEPLOYMENT_ORCHESTRATOR,
            ),
            Some("GET /targets/rp2040/browser-deployment/picoboot.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/picoboot.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_PICOBOOT,
            ),
            Some("GET /targets/avr/browser-deployment/creche-adapter.mjs HTTP/1.1")
            | Some("GET /creche/targets/avr/browser-deployment/creche-adapter.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::AVR_ADAPTER,
            ),
            Some("GET /targets/avr/browser-deployment/image.mjs HTTP/1.1")
            | Some("GET /creche/targets/avr/browser-deployment/image.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::AVR_IMAGE,
            ),
            Some("GET /targets/rp2040/browser-deployment/uf2.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/uf2.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_UF2,
            ),
            Some("GET /targets/rp2040/browser-deployment/bootsel.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/bootsel.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_BOOTSEL,
            ),
            Some("GET /targets/rp2040/browser-deployment/spawn.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/spawn.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_SPAWN,
            ),
            Some("GET /targets/rp2040/browser-deployment/fabrication.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/fabrication.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RP2040_FABRICATION,
            ),
            Some("GET /targets/rp2040/browser-deployment/creche-adapter.mjs HTTP/1.1")
            | Some("GET /creche/targets/rp2040/browser-deployment/creche-adapter.mjs HTTP/1.1") => {
                (
                    "200 OK",
                    "text/javascript; charset=utf-8",
                    creche_assets::RP2040_ADAPTER,
                )
            }
            Some("GET /targets/esp32/browser-deployment/index.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/index.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_DEPLOYMENT,
            ),
            Some("GET /targets/esp32/browser-deployment/deployment.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/deployment.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_DEPLOYMENT_ORCHESTRATOR,
            ),
            Some("GET /targets/esp32/browser-deployment/image.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/image.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_IMAGE,
            ),
            Some("GET /targets/esp32/browser-deployment/md5.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/md5.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_MD5,
            ),
            Some("GET /targets/esp32/browser-deployment/reset.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/reset.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_RESET,
            ),
            Some("GET /targets/esp32/browser-deployment/rom-loader.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/rom-loader.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_ROM_LOADER,
            ),
            Some("GET /targets/esp32/browser-deployment/slip.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/slip.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_SLIP,
            ),
            Some("GET /targets/esp32/browser-deployment/creche-adapter.mjs HTTP/1.1")
            | Some("GET /creche/targets/esp32/browser-deployment/creche-adapter.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::ESP32_ADAPTER,
            ),
            Some("GET /runtime.wasm HTTP/1.1")
            | Some("GET /book/runtime.wasm HTTP/1.1")
            | Some("GET /creche/runtime.wasm HTTP/1.1") => {
                ("200 OK", "application/wasm", self.runtime.as_slice())
            }
            Some("GET /book/book.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                book_assets::SCRIPT,
            ),
            Some("GET /book/book-state.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                book_assets::STATE,
            ),
            Some("GET /book/book-navigation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                book_assets::NAVIGATION,
            ),
            Some("GET /book/book-runner-presentation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                book_assets::RUNNER_PRESENTATION,
            ),
            Some("GET /book/book-syntax-editor.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                book_assets::SYNTAX_EDITOR,
            ),
            Some("GET /creche/creche-lifecycle.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::LIFECYCLE,
            ),
            Some("GET /creche/creche-physical.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::PHYSICAL,
            ),
            Some("GET /creche/creche-target-catalog.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::TARGET_CATALOG,
            ),
            Some(
                "GET /creche/targets/raspberry-pi/browser-deployment/creche-adapter.mjs HTTP/1.1",
            ) => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RASPBERRY_PI_ADAPTER,
            ),
            Some("GET /creche/targets/raspberry-pi/browser-deployment/image.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::RASPBERRY_PI_IMAGE,
            ),
            Some(
                "GET /creche/targets/conduitos/browser-deployment/creche-adapter.mjs HTTP/1.1",
            ) => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::CONDUITOS_ADAPTER,
            ),
            Some("GET /creche/targets/conduitos/browser-deployment/image.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::CONDUITOS_IMAGE,
            ),
            Some("GET /creche/creche-graduation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::GRADUATION,
            ),
            Some("GET /creche/creche.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                creche_assets::SCRIPT,
            ),
            Some("GET /creche/creche.css HTTP/1.1") => {
                ("200 OK", "text/css; charset=utf-8", creche_assets::STYLE)
            }
            Some("GET /creche/application-presentation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                APPLICATION_PRESENTATION,
            ),
            Some("GET /creche/artifacts/pico-w-signal-pico-local.json HTTP/1.1") => (
                "200 OK",
                "application/json; charset=utf-8",
                creche_assets::PICO_ARTIFACT_MANIFEST,
            ),
            Some("GET /creche/artifacts/pico-w-signal-pico-local.uf2 HTTP/1.1") => (
                "200 OK",
                "application/octet-stream",
                creche_assets::PICO_ARTIFACT,
            ),
            Some("GET /book/book.css HTTP/1.1") => {
                ("200 OK", "text/css; charset=utf-8", book_assets::STYLE)
            }
            Some("GET /book/chapter-1.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[0],
            ),
            Some("GET /book/chapter-2.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[1],
            ),
            Some("GET /book/chapter-3.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[2],
            ),
            Some("GET /book/chapter-4.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[3],
            ),
            Some("GET /book/chapter-5.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[4],
            ),
            Some("GET /book/chapter-6.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[5],
            ),
            Some("GET /book/chapter-8.md HTTP/1.1") => (
                "200 OK",
                "text/markdown; charset=utf-8",
                book_assets::CHAPTERS[6],
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
