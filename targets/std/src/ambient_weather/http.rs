//! Fixed-buffer HTTP mechanism for an independently admitted exact endpoint.
//! No DNS, redirects, credentials, scheduling or authority issuance during read.
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

pub const MAXIMUM_CURRENT_BODY_BYTES: usize = 4096;
const MAXIMUM_HEADERS: usize = 4096;
const MAXIMUM_WAIT: Duration = Duration::from_secs(2);
#[cfg(not(unix))]
const CONNECT_WAIT: Duration = Duration::from_millis(250);
#[cfg(unix)]
#[path = "http/connect.rs"]
mod connect;
#[cfg(unix)]
const POLL_WAIT: Duration = Duration::from_millis(25);
const REQUEST: &[u8] = b"GET /v1/forecast?latitude=47.6062&longitude=-122.3321&current=temperature_2m,weather_code&temperature_unit=celsius&timeformat=unixtime&timezone=UTC HTTP/1.0\r\nHost: api.open-meteo.com\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMeteoHttpRefusal {
    InvalidEndpoint,
    InvalidTick,
    Cancelled,
    Deadline,
    Connect,
    Io,
    Headers,
    Status(u16),
    Framing,
    ContentType,
    BodyBound,
    OutputBound,
}

/// Mechanism only. Preparation supplies the already resolved, independently
/// authorized peer. This object neither proves endpoint authorization nor
/// registers a Base; installation must bind its exact peer and HTTP profile.
/// HTTP is explicit, unauthenticated transport, with no automatic TLS fallback.
/// Unix connection and IO waits check cancellation at most every 25ms, sharing
/// the same two-second total deadline. Other targets retain a 250ms connect wait.
/// These are cooperative waits, not hard OS scheduling or RSS guarantees.
pub struct OpenMeteoHttp {
    endpoint: SocketAddr,
}
impl OpenMeteoHttp {
    pub fn prepare(endpoint: SocketAddr) -> Result<Self, OpenMeteoHttpRefusal> {
        if endpoint.port() == 0 || endpoint.ip().is_unspecified() || endpoint.ip().is_multicast() {
            return Err(OpenMeteoHttpRefusal::InvalidEndpoint);
        }
        Ok(Self { endpoint })
    }
    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    /// Caller must separately admit the platform effect, fixed stack, socket,
    /// output and cancellation custody. No request/frame identity is invented.
    pub fn read_into(
        &mut self,
        tick: &[u8],
        output: &mut [u8],
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> Result<usize, OpenMeteoHttpRefusal> {
        use OpenMeteoHttpRefusal as E;
        check(deadline, &cancelled)?;
        conduit_time::decode_tick(tick).map_err(|_| E::InvalidTick)?;
        let deadline = deadline.min(Instant::now() + MAXIMUM_WAIT);
        #[cfg(unix)]
        let mut stream = connect::connect(self.endpoint, deadline, &cancelled)?;
        #[cfg(not(unix))]
        let mut stream = {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(E::Deadline);
            }
            TcpStream::connect_timeout(&self.endpoint, remaining.min(CONNECT_WAIT))
                .map_err(|_| E::Connect)?
        };
        stream.set_nonblocking(true).map_err(|_| E::Io)?;
        let mut sent = 0;
        while sent < REQUEST.len() {
            check(deadline, &cancelled)?;
            match stream.write(&REQUEST[sent..]) {
                Ok(0) => return Err(E::Io),
                Ok(n) => sent += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    wait(&stream, true, deadline, &cancelled)?
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return Err(E::Io),
            }
        }
        let mut wire = [0u8; MAXIMUM_HEADERS + MAXIMUM_CURRENT_BODY_BYTES + 1];
        let mut length = 0;
        let mut header = None;
        loop {
            check(deadline, &cancelled)?;
            if length == wire.len() {
                return Err(E::BodyBound);
            }
            let eof = match stream.read(&mut wire[length..]) {
                Ok(0) => true,
                Ok(n) => {
                    length += n;
                    false
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    wait(&stream, false, deadline, &cancelled)?;
                    continue;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(E::Io),
            };
            if header.is_none() {
                if let Some(at) = wire[..length].windows(4).position(|v| v == b"\r\n\r\n") {
                    if at + 4 > MAXIMUM_HEADERS {
                        return Err(E::Headers);
                    }
                    header = Some((at + 4, headers(&wire[..at])?));
                } else if length >= MAXIMUM_HEADERS || eof {
                    return Err(E::Headers);
                }
            }
            if let Some((at, expected)) = header {
                let body = &wire[at..length];
                if body.len() > MAXIMUM_CURRENT_BODY_BYTES {
                    return Err(E::BodyBound);
                }
                let complete = match expected {
                    Some(n) if body.len() > n => return Err(E::Framing),
                    Some(n) => body.len() == n,
                    None => eof,
                };
                if eof && !complete {
                    return Err(E::Framing);
                }
                if complete {
                    check(deadline, &cancelled)?;
                    if body.is_empty() {
                        return Err(E::Framing);
                    }
                    let destination = output.get_mut(..body.len()).ok_or(E::OutputBound)?;
                    destination.copy_from_slice(body);
                    return Ok(body.len());
                }
            }
        }
    }
}

fn check(deadline: Instant, cancelled: &impl Fn() -> bool) -> Result<(), OpenMeteoHttpRefusal> {
    if cancelled() {
        Err(OpenMeteoHttpRefusal::Cancelled)
    } else if Instant::now() >= deadline {
        Err(OpenMeteoHttpRefusal::Deadline)
    } else {
        Ok(())
    }
}
#[cfg(unix)]
fn wait(
    stream: &TcpStream,
    write: bool,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<(), OpenMeteoHttpRefusal> {
    use std::os::fd::AsRawFd;
    check(deadline, cancelled)?;
    let mut fd = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: if write { libc::POLLOUT } else { libc::POLLIN },
        revents: 0,
    };
    let timeout = deadline
        .saturating_duration_since(Instant::now())
        .min(POLL_WAIT)
        .as_millis()
        .max(1) as i32;
    // SAFETY: one live borrowed descriptor, one initialized pollfd, finite wait.
    let result = unsafe { libc::poll(&mut fd, 1, timeout) };
    if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
        return Err(OpenMeteoHttpRefusal::Io);
    }
    check(deadline, cancelled)
}
#[cfg(not(unix))]
fn wait(
    _: &TcpStream,
    _: bool,
    _: Instant,
    _: &impl Fn() -> bool,
) -> Result<(), OpenMeteoHttpRefusal> {
    Err(OpenMeteoHttpRefusal::Io)
}

fn headers(bytes: &[u8]) -> Result<Option<usize>, OpenMeteoHttpRefusal> {
    use OpenMeteoHttpRefusal as E;
    let text = std::str::from_utf8(bytes).map_err(|_| E::Headers)?;
    let mut lines = text.split("\r\n");
    let mut status = lines.next().ok_or(E::Headers)?.split_ascii_whitespace();
    if !matches!(status.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err(E::Headers);
    }
    let status_digits = status.next().ok_or(E::Headers)?;
    if status_digits.len() != 3 || !status_digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(E::Headers);
    }
    let code = status_digits.parse::<u16>().map_err(|_| E::Headers)?;
    if code != 200 {
        return Err(E::Status(code));
    }
    let mut length = None;
    let mut json = false;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(E::Headers)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(E::Framing);
            }
            if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
                return Err(E::Framing);
            }
            let n = value.parse::<usize>().map_err(|_| E::Framing)?;
            if n > MAXIMUM_CURRENT_BODY_BYTES {
                return Err(E::BodyBound);
            }
            length = Some(n);
        } else if name.eq_ignore_ascii_case("content-type") {
            if json
                || !value
                    .split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            {
                return Err(E::ContentType);
            }
            json = true;
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            || (name.eq_ignore_ascii_case("content-encoding")
                && !value.eq_ignore_ascii_case("identity"))
        {
            return Err(E::Framing);
        }
    }
    if !json {
        return Err(E::ContentType);
    }
    Ok(length)
}

// SAFETY: reads use a fixed request and stack/output buffers, one preselected
// peer, no DNS/formatting/heap work and bounded connect/poll waits. The actual
// installer still owes matching independent authority and native/socket budgets.
#[cfg(target_os = "linux")]
unsafe impl crate::NativeSourceWorkerProvider for OpenMeteoHttp {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &crate::NativeReadContext<'_>,
    ) -> Result<usize, crate::WorkerFailure> {
        self.read_into(input, output, context.deadline(), || context.cancelled())
            .map_err(|e| match e {
                OpenMeteoHttpRefusal::Cancelled => crate::WorkerFailure::Cancelled,
                OpenMeteoHttpRefusal::Deadline => crate::WorkerFailure::TimedOut,
                OpenMeteoHttpRefusal::BodyBound | OpenMeteoHttpRefusal::OutputBound => {
                    crate::WorkerFailure::InvalidOutput
                }
                _ => crate::WorkerFailure::Provider,
            })
    }
}

#[cfg(test)]
mod tests;
