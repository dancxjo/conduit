//! Finite loopback delivery for one standalone browser Host page.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;

const INDEX: &[u8] = include_bytes!("../assets/index.html");
const BOOTSTRAP: &[u8] = include_bytes!("../assets/host.mjs");
const HOST_BOOTSTRAP: &[u8] = include_bytes!("../assets/browser-host-bootstrap.mjs");
const MEDIA_HOST: &[u8] = include_bytes!("../assets/media-host.mjs");
const DEVICE_BASE: &[u8] = include_bytes!("../assets/device-base.mjs");
const USB_DEVICE_BASE: &[u8] = include_bytes!("../assets/usb-device-base.mjs");
const RP2040_DEPLOYMENT: &[u8] = include_bytes!("../../../rp2040/browser-deployment/index.mjs");
const RP2040_DEPLOYMENT_ORCHESTRATOR: &[u8] =
    include_bytes!("../../../rp2040/browser-deployment/deployment.mjs");
const RP2040_PICOBOOT: &[u8] = include_bytes!("../../../rp2040/browser-deployment/picoboot.mjs");
const RP2040_UF2: &[u8] = include_bytes!("../../../rp2040/browser-deployment/uf2.mjs");
const RP2040_BOOTSEL: &[u8] = include_bytes!("../../../rp2040/browser-deployment/bootsel.mjs");
const RP2040_SPAWN: &[u8] = include_bytes!("../../../rp2040/browser-deployment/spawn.mjs");
const ESP32_DEPLOYMENT: &[u8] = include_bytes!("../../../esp32/browser-deployment/index.mjs");
const ESP32_DEPLOYMENT_ORCHESTRATOR: &[u8] =
    include_bytes!("../../../esp32/browser-deployment/deployment.mjs");
const ESP32_IMAGE: &[u8] = include_bytes!("../../../esp32/browser-deployment/image.mjs");
const ESP32_MD5: &[u8] = include_bytes!("../../../esp32/browser-deployment/md5.mjs");
const ESP32_RESET: &[u8] = include_bytes!("../../../esp32/browser-deployment/reset.mjs");
const ESP32_ROM_LOADER: &[u8] = include_bytes!("../../../esp32/browser-deployment/rom-loader.mjs");
const ESP32_SLIP: &[u8] = include_bytes!("../../../esp32/browser-deployment/slip.mjs");
const BOOK: &[u8] = include_bytes!("../assets/book.html");
const BOOK_SCRIPT: &[u8] = include_bytes!("../assets/book.mjs");
const BOOK_LIFECYCLE_SCRIPT: &[u8] = include_bytes!("../assets/book-lifecycle.mjs");
const BOOK_PHYSICAL_SCRIPT: &[u8] = include_bytes!("../assets/book-physical.mjs");
const BOOK_GRADUATION_SCRIPT: &[u8] = include_bytes!("../assets/book-graduation.mjs");
const BOOK_STYLE: &[u8] = include_bytes!("../assets/book.css");
const BOOK_CHAPTER_ONE: &[u8] = include_bytes!("../../../../tour/book/chapter-1.md");
const BOOK_CHAPTER_TWO: &[u8] = include_bytes!("../../../../tour/book/chapter-2.md");
const BOOK_CHAPTER_THREE: &[u8] = include_bytes!("../../../../tour/book/chapter-3.md");
const BOOK_CHAPTER_FOUR: &[u8] = include_bytes!("../../../../tour/book/chapter-4.md");
const BOOK_CHAPTER_FIVE: &[u8] = include_bytes!("../../../../tour/book/chapter-5.md");
const BOOK_CHAPTER_SIX: &[u8] = include_bytes!("../../../../tour/book/chapter-6.md");
const BOOK_CHAPTER_SEVEN: &[u8] = include_bytes!("../../../../tour/book/chapter-7.md");
const BOOK_CHAPTER_EIGHT: &[u8] = include_bytes!("../../../../tour/book/chapter-8.md");
const MAX_RUNTIME_BYTES: usize = 4 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 4096;
const MAX_REQUESTS: usize = 1024;

#[derive(Debug)]
pub struct BrowserHostServer {
    listener: TcpListener,
    runtime: Vec<u8>,
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
        Ok(Self { listener, runtime })
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
        let (status, content_type, body): (&str, &str, &[u8]) = match request.lines().next() {
            Some("GET / HTTP/1.1") => ("200 OK", "text/html; charset=utf-8", INDEX),
            Some("GET /host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", BOOTSTRAP)
            }
            Some("GET /browser-host-bootstrap.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", HOST_BOOTSTRAP)
            }
            Some("GET /media-host.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", MEDIA_HOST)
            }
            Some("GET /device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", DEVICE_BASE)
            }
            Some("GET /usb-device-base.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", USB_DEVICE_BASE)
            }
            Some("GET /targets/rp2040/browser-deployment/index.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                RP2040_DEPLOYMENT,
            ),
            Some("GET /targets/rp2040/browser-deployment/deployment.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                RP2040_DEPLOYMENT_ORCHESTRATOR,
            ),
            Some("GET /targets/rp2040/browser-deployment/picoboot.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", RP2040_PICOBOOT)
            }
            Some("GET /targets/rp2040/browser-deployment/uf2.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", RP2040_UF2)
            }
            Some("GET /targets/rp2040/browser-deployment/bootsel.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", RP2040_BOOTSEL)
            }
            Some("GET /targets/rp2040/browser-deployment/spawn.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", RP2040_SPAWN)
            }
            Some("GET /targets/esp32/browser-deployment/index.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_DEPLOYMENT)
            }
            Some("GET /targets/esp32/browser-deployment/deployment.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                ESP32_DEPLOYMENT_ORCHESTRATOR,
            ),
            Some("GET /targets/esp32/browser-deployment/image.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_IMAGE)
            }
            Some("GET /targets/esp32/browser-deployment/md5.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_MD5)
            }
            Some("GET /targets/esp32/browser-deployment/reset.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_RESET)
            }
            Some("GET /targets/esp32/browser-deployment/rom-loader.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_ROM_LOADER)
            }
            Some("GET /targets/esp32/browser-deployment/slip.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", ESP32_SLIP)
            }
            Some("GET /runtime.wasm HTTP/1.1") => {
                ("200 OK", "application/wasm", self.runtime.as_slice())
            }
            Some("GET /book/ HTTP/1.1") => ("200 OK", "text/html; charset=utf-8", BOOK),
            Some("GET /book/book.mjs HTTP/1.1") => {
                ("200 OK", "text/javascript; charset=utf-8", BOOK_SCRIPT)
            }
            Some("GET /book/book-lifecycle.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BOOK_LIFECYCLE_SCRIPT,
            ),
            Some("GET /book/book-physical.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BOOK_PHYSICAL_SCRIPT,
            ),
            Some("GET /book/book-graduation.mjs HTTP/1.1") => (
                "200 OK",
                "text/javascript; charset=utf-8",
                BOOK_GRADUATION_SCRIPT,
            ),
            Some("GET /book/book.css HTTP/1.1") => {
                ("200 OK", "text/css; charset=utf-8", BOOK_STYLE)
            }
            Some("GET /book/chapter-1.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_ONE)
            }
            Some("GET /book/chapter-2.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_TWO)
            }
            Some("GET /book/chapter-3.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_THREE)
            }
            Some("GET /book/chapter-4.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_FOUR)
            }
            Some("GET /book/chapter-5.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_FIVE)
            }
            Some("GET /book/chapter-6.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_SIX)
            }
            Some("GET /book/chapter-7.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_SEVEN)
            }
            Some("GET /book/chapter-8.md HTTP/1.1") => {
                ("200 OK", "text/markdown; charset=utf-8", BOOK_CHAPTER_EIGHT)
            }
            _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found"),
        };
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
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn runtime_fixture() -> std::path::PathBuf {
        let name = format!(
            "conduit-browser-host-runtime-{}-{}.wasm",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"bounded wasm fixture").unwrap();
        path
    }

    #[test]
    fn simultaneous_entrances_are_distinct_ipv4_loopback_listeners() {
        let runtime = runtime_fixture();
        let first = BrowserHostServer::bind(&runtime).unwrap();
        let second = BrowserHostServer::bind(&runtime).unwrap();
        let first_address = first.local_addr().unwrap();
        let second_address = second.local_addr().unwrap();
        assert_eq!(first_address.ip(), Ipv4Addr::LOCALHOST);
        assert_eq!(second_address.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(first_address, second_address);
        std::fs::remove_file(runtime).unwrap();
    }

    #[test]
    fn missing_and_oversized_runtime_artifacts_refuse_before_launch() {
        let missing = std::env::temp_dir().join("conduit-browser-host-absent.wasm");
        assert!(BrowserHostServer::bind(&missing)
            .unwrap_err()
            .contains("unavailable"));

        let runtime = runtime_fixture();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&runtime)
            .unwrap();
        file.set_len(MAX_RUNTIME_BYTES as u64 + 1).unwrap();
        assert!(BrowserHostServer::bind(&runtime)
            .unwrap_err()
            .contains("artifact bound"));
        std::fs::remove_file(runtime).unwrap();
    }
}
