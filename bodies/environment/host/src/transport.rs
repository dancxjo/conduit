//! Bounded raw UART carrier for the existing Root protocol; no policy lives here.
use crate::Result;
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    time::{Duration, Instant},
};
#[path = "transport_error.rs"]
mod error;
#[path = "transport_fault.rs"]
mod fault;
pub use error::retryable as is_recoverable;
pub struct Line {
    file: File,
    endpoint: Option<std::net::SocketAddr>,
}
impl Line {
    pub fn open(path: &str) -> Result<Self> {
        if let Some(address) = path.strip_prefix("tcp://") {
            use std::os::fd::{FromRawFd, IntoRawFd};
            let endpoint = address.parse()?;
            let stream = std::net::TcpStream::connect_timeout(&endpoint, Duration::from_secs(10))
                .map_err(error::io)?;
            stream.set_nodelay(true).map_err(error::io)?;
            stream.set_nonblocking(true).map_err(error::io)?;
            // SAFETY: transfer the sole owned descriptor; no stream alias remains.
            let file = unsafe { File::from_raw_fd(stream.into_raw_fd()) };
            return Ok(Self {
                file,
                endpoint: Some(endpoint),
            });
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        let fd = file.as_raw_fd();
        // SAFETY: the initialized termios object belongs to this open UART descriptor.
        unsafe {
            let mut termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut termios) != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            libc::cfmakeraw(&mut termios);
            libc::cfsetispeed(&mut termios, libc::B115200);
            libc::cfsetospeed(&mut termios, libc::B115200);
            termios.c_cflag |= libc::CLOCAL | libc::CREAD;
            termios.c_cflag &= !libc::CRTSCTS;
            if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0
                || libc::ioctl(fd, libc::TIOCEXCL) != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(Self {
            file,
            endpoint: None,
        })
    }
    pub fn can_reconnect(&self) -> bool {
        self.endpoint.is_some()
    }
    /// Reopen only this explicitly selected TCP peer. The management owner must
    /// separately admit retry lifetime and authenticate the retained Body scope.
    pub fn reconnect(&mut self) -> Result<()> {
        let endpoint = self.endpoint.ok_or("UART has no implicit reconnect")?;
        let fd = std::os::fd::OwnedFd::from(self.file.try_clone().map_err(error::io)?);
        // Close the failed carrier in both directions so Root can retire its
        // partial framing and accept another one; resident workers are separate.
        let _ = std::net::TcpStream::from(fd).shutdown(std::net::Shutdown::Both);
        *self = Self::open(&format!("tcp://{endpoint}"))?;
        Ok(())
    }
    /// Read-only HTTP replay uses the already selected TCP peer, never an
    /// ambient hostname or another Body member. UART has no implied HTTP line.
    pub fn http_peer(&self) -> Option<std::net::SocketAddr> {
        let fd = std::os::fd::OwnedFd::from(self.file.try_clone().ok()?);
        let mut address = std::net::TcpStream::from(fd).peer_addr().ok()?;
        address.set_port(80);
        Some(address)
    }
    fn ready(&self, event: i16, deadline: Instant) -> Result<()> {
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(error::timeout)?;
            let mut fd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: event,
                revents: 0,
            };
            // SAFETY: one valid pollfd is supplied for this finite wait.
            let n = unsafe {
                libc::poll(
                    &mut fd,
                    1,
                    remaining.as_millis().min(i32::MAX as u128).max(1) as i32,
                )
            };
            if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            if n <= 0 || fd.revents & event == 0 {
                return Err(if n == 0 {
                    error::timeout()
                } else {
                    error::closed("owner carrier disconnected")
                });
            }
            return Ok(());
        }
    }
    pub fn write(&mut self, mut bytes: &[u8]) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !bytes.is_empty() {
            self.ready(libc::POLLOUT, deadline)?;
            let n = self.file.write(bytes).map_err(error::io)?;
            if n == 0 {
                return Err(error::closed("owner carrier write disconnected"));
            }
            bytes = &bytes[n..];
        }
        Ok(())
    }
    pub fn text(&mut self, text: &str) -> Result<()> {
        self.write(text.as_bytes())?;
        self.write(b"\n")
    }
    pub fn exact(&mut self, size: usize) -> Result<Vec<u8>> {
        if size > 64 * 1024 {
            return Err("UART reply bound".into());
        }
        let mut out = vec![0; size];
        let mut at = 0;
        let deadline = Instant::now() + Duration::from_secs(15);
        while at < size {
            self.ready(libc::POLLIN, deadline)?;
            let n = self.file.read(&mut out[at..]).map_err(error::io)?;
            if n == 0 {
                return Err(error::closed("owner carrier read disconnected"));
            }
            at += n;
        }
        Ok(out)
    }
    pub fn event(&mut self, name: &str) -> Result<Value> {
        let mut faults = fault::RecentFaults::default();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut bytes = Vec::new();
        for _ in 0..128 * 1024 {
            self.ready(libc::POLLIN, deadline)
                .map_err(|error| faults.describe(name, error))?;
            let mut byte = [0];
            if self.file.read(&mut byte).map_err(error::io)? == 0 {
                return Err(error::closed("owner carrier closed"));
            }
            if byte[0] == b'\n' {
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    if value["event"] == name {
                        return Ok(value);
                    }
                    if value["event"] == "native-root-request-refused" {
                        return Err("Root refused command".into());
                    }
                } else {
                    faults.remember(&bytes);
                }
                bytes.clear();
            } else {
                if bytes.len() == 64 * 1024 {
                    return Err("UART line bound".into());
                }
                bytes.push(byte[0]);
            }
        }
        Err("UART exchange bound".into())
    }
    pub fn document(&mut self) -> Result<Value> {
        // Admission documents have no event discriminator and fit Root's 8 KiB bound.
        let mut bytes = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        for _ in 0..8192 {
            self.ready(libc::POLLIN, deadline)?;
            let mut byte = [0];
            self.file.read_exact(&mut byte).map_err(error::io)?;
            if byte[0] == b'\n' {
                if let Ok(value) = serde_json::from_slice(&bytes) {
                    return Ok(value);
                }
                bytes.clear();
            } else {
                bytes.push(byte[0]);
            }
        }
        Err("admission document bound".into())
    }
    pub fn command(&mut self, command: &str, event: &str) -> Result<Value> {
        self.text(command)?;
        self.event(event)
    }
    pub fn credited(&mut self, bytes: &[u8]) -> Result<()> {
        for chunk in bytes.chunks(64) {
            self.event("native-owner-ingress-credit")?;
            self.write(chunk)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
