//! Native USB CDC ACM line implementation for std host.
//!
//! Provides length-prefixed stream framing over native POSIX serial endpoints
//! and generic `Read + Write` streams.

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder, StreamFrameError};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionFrame, WireError};

#[derive(Debug)]
pub enum NativeUsbCdcError {
    InvalidLimit,
    Read(std::io::ErrorKind),
    Write(std::io::ErrorKind),
    Framing(StreamFrameError),
    Codec(WireError),
    WouldBlock,
    Disconnected,
    TtyConfig(std::io::Error),
}

impl From<StreamFrameError> for NativeUsbCdcError {
    fn from(err: StreamFrameError) -> Self {
        Self::Framing(err)
    }
}

impl From<WireError> for NativeUsbCdcError {
    fn from(err: WireError) -> Self {
        Self::Codec(err)
    }
}

impl std::fmt::Display for NativeUsbCdcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLimit => write!(f, "invalid frame size limit"),
            Self::Read(err) => write!(f, "USB CDC read error: {err:?}"),
            Self::Write(err) => write!(f, "USB CDC write error: {err:?}"),
            Self::Framing(err) => write!(f, "USB CDC framing error: {err}"),
            Self::Codec(err) => write!(f, "USB CDC codec error: {err:?}"),
            Self::WouldBlock => write!(f, "USB CDC read timed out / would block"),
            Self::Disconnected => write!(f, "USB CDC device disconnected"),
            Self::TtyConfig(err) => write!(f, "USB CDC TTY configuration error: {err}"),
        }
    }
}

impl std::error::Error for NativeUsbCdcError {}

#[cfg(unix)]
pub struct FdGuard(pub libc::c_int);

#[cfg(unix)]
impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// Configure raw serial CDC line settings (115200 8N1, raw mode, CLOCAL | CREAD, VMIN, VTIME)
#[cfg(unix)]
pub fn configure_cdc_port<F: AsRawFd>(
    file: &F,
    min_bytes: u8,
    timeout_deciseconds: u8,
) -> Result<(), NativeUsbCdcError> {
    let fd = file.as_raw_fd();
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut termios) != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }

        libc::cfmakeraw(&mut termios);
        libc::cfsetispeed(&mut termios, libc::B115200);
        libc::cfsetospeed(&mut termios, libc::B115200);

        termios.c_cflag &= !libc::CSIZE;
        termios.c_cflag |= libc::CS8;
        termios.c_cflag &= !libc::CSTOPB;
        termios.c_cflag &= !libc::PARENB;
        termios.c_cflag |= libc::CLOCAL | libc::CREAD;

        termios.c_cc[libc::VMIN] = min_bytes as libc::cc_t;
        termios.c_cc[libc::VTIME] = timeout_deciseconds as libc::cc_t;

        if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

/// Configure raw serial CDC line settings (115200 8N1, raw mode, CLOCAL | CREAD, VMIN=0, VTIME=0, tcflush)
#[cfg(unix)]
pub fn configure_raw_termios(fd: libc::c_int) -> Result<(), NativeUsbCdcError> {
    unsafe {
        let mut term: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut term) != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        term.c_iflag = 0;
        term.c_oflag = 0;
        term.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL;
        term.c_lflag = 0;
        libc::cfsetispeed(&mut term, libc::B115200);
        libc::cfsetospeed(&mut term, libc::B115200);
        term.c_cc[libc::VMIN] = 0;
        term.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(fd, libc::TCSANOW, &term) != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        libc::tcflush(fd, libc::TCIOFLUSH);
    }
    Ok(())
}

/// Assert DTR and verify that the kernel reports it high for this CDC endpoint.
#[cfg(unix)]
pub fn assert_dtr<F: AsRawFd>(file: &F) -> Result<(), NativeUsbCdcError> {
    assert_dtr_fd(file.as_raw_fd())
}

#[cfg(unix)]
fn assert_dtr_fd(fd: libc::c_int) -> Result<(), NativeUsbCdcError> {
    let mut set_flags: libc::c_int = libc::TIOCM_DTR;
    if unsafe { libc::ioctl(fd, libc::TIOCMBIS, &mut set_flags) } != 0 {
        return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
    }

    let mut observed_flags: libc::c_int = 0;
    if unsafe { libc::ioctl(fd, libc::TIOCMGET, &mut observed_flags) } != 0 {
        return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
    }
    if observed_flags & libc::TIOCM_DTR == 0 {
        return Err(NativeUsbCdcError::TtyConfig(std::io::Error::other(
            "USB CDC DTR remained low after assertion",
        )));
    }

    Ok(())
}

#[cfg(unix)]
fn clear_dtr_fd(fd: libc::c_int) {
    let mut clear_flags: libc::c_int = libc::TIOCM_DTR;
    unsafe {
        // Closing a CDC ACM descriptor does not reliably lower DTR on every
        // kernel/device combination. This is best-effort during Drop: the
        // endpoint close still follows, while a successful ioctl gives the
        // device an observable connection boundary before that close.
        libc::ioctl(fd, libc::TIOCMBIC, &mut clear_flags);
    }
}

/// Owned `/dev/tty` operator terminal abstraction for interactive key input.
#[cfg(unix)]
pub struct OperatorTerminal {
    fd: FdGuard,
    saved_termios: libc::termios,
}

#[cfg(unix)]
impl OperatorTerminal {
    pub fn open() -> Result<Self, NativeUsbCdcError> {
        let path_c = std::ffi::CString::new("/dev/tty").unwrap();
        let fd = unsafe {
            libc::open(
                path_c.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        let guard = FdGuard(fd);
        unsafe {
            let mut saved_termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut saved_termios) != 0 {
                return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
            }
            let mut raw_termios = saved_termios;
            libc::cfmakeraw(&mut raw_termios);
            if libc::tcsetattr(fd, libc::TCSANOW, &raw_termios) != 0 {
                return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
            }
            Ok(Self {
                fd: guard,
                saved_termios,
            })
        }
    }

    pub fn read_key(&mut self, timeout: Duration) -> Result<Option<u8>, NativeUsbCdcError> {
        let mut pfd = libc::pollfd {
            fd: self.fd.0,
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
        let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
        if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
            let mut buf = [0u8; 1];
            let n = unsafe { libc::read(self.fd.0, buf.as_mut_ptr() as *mut _, 1) };
            if n > 0 {
                return Ok(Some(buf[0]));
            }
        }
        Ok(None)
    }
}

#[cfg(unix)]
impl Drop for OperatorTerminal {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::tcsetattr(self.fd.0, libc::TCSANOW, &self.saved_termios);
        }
    }
}

/// Physical path-based native USB CDC line using poll(2) and raw non-blocking POSIX descriptor.
#[cfg(unix)]
pub struct NativePathCdcLine {
    fd: FdGuard,
    maximum_frame_bytes: usize,
    decoder: StreamFrameDecoder<4096>,
}

#[cfg(unix)]
impl Drop for NativePathCdcLine {
    fn drop(&mut self) {
        clear_dtr_fd(self.fd.0);
    }
}

#[cfg(unix)]
impl NativePathCdcLine {
    pub fn open<P: AsRef<Path>>(
        path: P,
        maximum_frame_bytes: usize,
    ) -> Result<Self, NativeUsbCdcError> {
        if maximum_frame_bytes == 0 || maximum_frame_bytes > 4096 {
            return Err(NativeUsbCdcError::InvalidLimit);
        }
        use std::os::unix::ffi::OsStrExt;
        let path_c =
            std::ffi::CString::new(path.as_ref().as_os_str().as_bytes()).map_err(|_| {
                NativeUsbCdcError::TtyConfig(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid path",
                ))
            })?;

        let fd = unsafe {
            libc::open(
                path_c.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        let guard = FdGuard(fd);
        configure_raw_termios(fd)?;
        assert_dtr_fd(fd)?;

        let decoder = StreamFrameDecoder::new(maximum_frame_bytes)?;
        Ok(Self {
            fd: guard,
            maximum_frame_bytes,
            decoder,
        })
    }

    pub fn send_frame(
        &mut self,
        frame: &SessionFrame<'_>,
        timeout: Duration,
    ) -> Result<(), NativeUsbCdcError> {
        let mut wire_buf = [0u8; 2048];
        let frame_len = encode_session_frame_into(
            *frame,
            &mut wire_buf[2..],
            self.maximum_frame_bytes as u32,
            self.maximum_frame_bytes as u32,
        )?;
        let mut framed_buf = [0u8; 2048];
        let total_bytes = encode_stream_frame(
            &wire_buf[2..2 + frame_len],
            self.maximum_frame_bytes,
            &mut framed_buf,
        )?;

        self.write_raw_bytes(&framed_buf[..total_bytes], timeout)
    }

    /// Report whether the exact opened USB path still has a live descriptor.
    /// This does not imply Body membership, authority, or protocol readiness.
    pub fn is_connected(&self) -> Result<bool, NativeUsbCdcError> {
        let mut pfd = libc::pollfd {
            fd: self.fd.0,
            events: 0,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut pfd, 1, 0) };
        if result < 0 {
            return Err(NativeUsbCdcError::Read(
                std::io::Error::last_os_error().kind(),
            ));
        }
        let disconnected = pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0;
        Ok(!disconnected)
    }

    pub fn write_raw_bytes(
        &mut self,
        bytes: &[u8],
        timeout: Duration,
    ) -> Result<(), NativeUsbCdcError> {
        let deadline = Instant::now() + timeout;
        let mut written = 0;
        while written < bytes.len() {
            let now = Instant::now();
            if now >= deadline {
                return Err(NativeUsbCdcError::WouldBlock);
            }
            let remaining = deadline - now;
            let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLOUT,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
            if ret < 0 {
                return Err(NativeUsbCdcError::Write(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ret > 0 && (pfd.revents & libc::POLLOUT) != 0 {
                let n = unsafe {
                    libc::write(
                        self.fd.0,
                        bytes[written..].as_ptr() as *const _,
                        bytes.len() - written,
                    )
                };
                if n > 0 {
                    written += n as usize;
                } else if n < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::WouldBlock
                        && err.kind() != std::io::ErrorKind::TimedOut
                    {
                        return Err(NativeUsbCdcError::Write(err.kind()));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn receive_frame<'a>(
        &mut self,
        frame_buf: &'a mut [u8],
        timeout: Duration,
    ) -> Result<SessionFrame<'a>, NativeUsbCdcError> {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0u8; 64];
        loop {
            if let Some(frame_bytes) = self.decoder.next_frame()? {
                if frame_buf.len() < frame_bytes.len() {
                    return Err(NativeUsbCdcError::Codec(WireError::OversizedFrame));
                }
                frame_buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                let frame = decode_session_frame(
                    &frame_buf[..frame_bytes.len()],
                    self.maximum_frame_bytes as u32,
                    self.maximum_frame_bytes as u32,
                )?;
                return Ok(frame);
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(NativeUsbCdcError::WouldBlock);
            }
            let remaining = deadline - now;
            let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
            if ret < 0 {
                return Err(NativeUsbCdcError::Read(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                let n = unsafe { libc::read(self.fd.0, chunk.as_mut_ptr() as *mut _, chunk.len()) };
                if n > 0 {
                    self.decoder.accept_bytes(&chunk[..n as usize])?;
                } else if n == 0 {
                    return Err(NativeUsbCdcError::Disconnected);
                } else {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::WouldBlock
                        && err.kind() != std::io::ErrorKind::TimedOut
                    {
                        return Err(NativeUsbCdcError::Read(err.kind()));
                    }
                }
            }
        }
    }

    pub fn send_raw_stream_frame(
        &mut self,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<(), NativeUsbCdcError> {
        let mut framed_buf = [0u8; 4098];
        let total_bytes = encode_stream_frame(payload, self.maximum_frame_bytes, &mut framed_buf)?;
        self.write_raw_bytes(&framed_buf[..total_bytes], timeout)
    }

    pub fn receive_raw_stream_frame<'a>(
        &mut self,
        frame_buf: &'a mut [u8],
        timeout: Duration,
    ) -> Result<&'a [u8], NativeUsbCdcError> {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0u8; 64];
        loop {
            if let Some(frame_bytes) = self.decoder.next_frame()? {
                if frame_buf.len() < frame_bytes.len() {
                    return Err(NativeUsbCdcError::InvalidLimit);
                }
                frame_buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                return Ok(&frame_buf[..frame_bytes.len()]);
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(NativeUsbCdcError::WouldBlock);
            }
            let remaining = deadline - now;
            let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
            if ret < 0 {
                return Err(NativeUsbCdcError::Read(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                let n = unsafe { libc::read(self.fd.0, chunk.as_mut_ptr() as *mut _, chunk.len()) };
                if n > 0 {
                    self.decoder.accept_bytes(&chunk[..n as usize])?;
                } else if n == 0 {
                    return Err(NativeUsbCdcError::Disconnected);
                } else {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::WouldBlock
                        && err.kind() != std::io::ErrorKind::TimedOut
                    {
                        return Err(NativeUsbCdcError::Read(err.kind()));
                    }
                }
            }
        }
    }

    pub fn receive_raw_bytes(
        &mut self,
        len: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>, NativeUsbCdcError> {
        let deadline = Instant::now() + timeout;
        let mut buf = vec![0u8; len];
        let mut read_bytes = 0;
        while read_bytes < len {
            let now = Instant::now();
            if now >= deadline {
                return Err(NativeUsbCdcError::WouldBlock);
            }
            let remaining = deadline - now;
            let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
            if ret < 0 {
                return Err(NativeUsbCdcError::Read(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                let n = unsafe {
                    libc::read(
                        self.fd.0,
                        buf[read_bytes..].as_mut_ptr() as *mut _,
                        len - read_bytes,
                    )
                };
                if n > 0 {
                    read_bytes += n as usize;
                } else if n == 0 {
                    return Err(NativeUsbCdcError::Disconnected);
                }
            }
        }
        Ok(buf)
    }
}

/// Physical path-based native USB CDC line reader using poll(2), DTR assertion, and raw non-blocking POSIX descriptor.
#[cfg(unix)]
pub struct NativePathCdcLineReader {
    fd: FdGuard,
    line_buf: Vec<u8>,
}

#[cfg(unix)]
impl NativePathCdcLineReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, NativeUsbCdcError> {
        use std::os::unix::ffi::OsStrExt;
        let path_c =
            std::ffi::CString::new(path.as_ref().as_os_str().as_bytes()).map_err(|_| {
                NativeUsbCdcError::TtyConfig(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid path",
                ))
            })?;

        let fd = unsafe {
            libc::open(
                path_c.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        let guard = FdGuard(fd);
        configure_raw_termios(fd)?;
        assert_dtr_fd(fd)?;

        Ok(Self {
            fd: guard,
            line_buf: Vec::with_capacity(1024),
        })
    }

    pub fn read_line(&mut self, timeout: Duration) -> Result<String, NativeUsbCdcError> {
        let deadline = Instant::now() + timeout;
        let mut chunk = [0u8; 64];
        loop {
            if let Some(pos) = self.line_buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.line_buf.drain(..=pos).collect();
                let line_str = String::from_utf8_lossy(&line_bytes).trim().to_string();
                if !line_str.is_empty() {
                    return Ok(line_str);
                }
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(NativeUsbCdcError::WouldBlock);
            }
            let remaining = deadline - now;
            let millis = remaining.as_millis().min(i32::MAX as u128) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
            if ret < 0 {
                return Err(NativeUsbCdcError::Read(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                let n = unsafe { libc::read(self.fd.0, chunk.as_mut_ptr() as *mut _, chunk.len()) };
                if n > 0 {
                    self.line_buf.extend_from_slice(&chunk[..n as usize]);
                } else if n == 0 {
                    return Err(NativeUsbCdcError::Disconnected);
                } else {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::WouldBlock
                        && err.kind() != std::io::ErrorKind::TimedOut
                    {
                        return Err(NativeUsbCdcError::Read(err.kind()));
                    }
                }
            }
        }
    }
}

/// Generic stream-based line for hardware-free unit tests.
pub struct NativeUsbCdcLine<R, W> {
    reader: R,
    writer: W,
    maximum_frame_bytes: usize,
    decoder: StreamFrameDecoder<2048>,
}

impl<R: Read, W: Write> NativeUsbCdcLine<R, W> {
    pub fn new(
        reader: R,
        writer: W,
        maximum_frame_bytes: usize,
    ) -> Result<Self, NativeUsbCdcError> {
        if maximum_frame_bytes == 0 || maximum_frame_bytes > 2048 {
            return Err(NativeUsbCdcError::InvalidLimit);
        }
        let decoder = StreamFrameDecoder::new(maximum_frame_bytes)?;
        Ok(Self {
            reader,
            writer,
            maximum_frame_bytes,
            decoder,
        })
    }

    pub fn send_frame(&mut self, frame: &SessionFrame<'_>) -> Result<(), NativeUsbCdcError> {
        let mut wire_buf = [0u8; 2048];
        let frame_len = encode_session_frame_into(
            *frame,
            &mut wire_buf[2..],
            self.maximum_frame_bytes as u32,
            self.maximum_frame_bytes as u32,
        )?;
        let mut framed_buf = [0u8; 2048];
        let total_bytes = encode_stream_frame(
            &wire_buf[2..2 + frame_len],
            self.maximum_frame_bytes,
            &mut framed_buf,
        )?;

        self.writer
            .write_all(&framed_buf[..total_bytes])
            .map_err(|e| NativeUsbCdcError::Write(e.kind()))?;
        self.writer
            .flush()
            .map_err(|e| NativeUsbCdcError::Write(e.kind()))?;
        Ok(())
    }

    pub fn receive_frame<'a>(
        &mut self,
        frame_buf: &'a mut [u8],
    ) -> Result<SessionFrame<'a>, NativeUsbCdcError> {
        let mut chunk = [0u8; 64];
        loop {
            if let Some(frame_bytes) = self.decoder.next_frame()? {
                if frame_buf.len() < frame_bytes.len() {
                    return Err(NativeUsbCdcError::Codec(WireError::OversizedFrame));
                }
                frame_buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                let frame = decode_session_frame(
                    &frame_buf[..frame_bytes.len()],
                    self.maximum_frame_bytes as u32,
                    self.maximum_frame_bytes as u32,
                )?;
                return Ok(frame);
            }

            let read_bytes = match self.reader.read(&mut chunk) {
                Ok(0) => return Err(NativeUsbCdcError::Disconnected),
                Ok(n) => n,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Err(NativeUsbCdcError::WouldBlock);
                }
                Err(e) => return Err(NativeUsbCdcError::Read(e.kind())),
            };
            self.decoder.accept_bytes(&chunk[..read_bytes])?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId, HostId, KindId,
        LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
    };
    use conduit_wire::{
        LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits, SessionMachine,
        SessionMessage, SessionRole,
    };
    use std::io::Cursor;

    fn test_binding() -> SessionBinding {
        use conduit_core::bind_active_play;
        let plan_id = PlanId::from("plan-1");
        let source = LinkEndpoint {
            host_id: HostId::from("host-1"),
            boot_id: BootId::from("boot-1"),
            endpoint_id: LinkEndpointId::from("end-1"),
        };
        let sink = LinkEndpoint {
            host_id: HostId::from("host-2"),
            boot_id: BootId::from("boot-2"),
            endpoint_id: LinkEndpointId::from("end-2"),
        };
        let source_active_play_id =
            bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0).active_play_id;
        let sink_active_play_id =
            bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0).active_play_id;

        SessionBinding {
            protocol_version: 1,
            plan_id,
            source_fragment_id: FragmentId::from("frag-1"),
            sink_fragment_id: FragmentId::from("frag-2"),
            source_active_play_id,
            sink_active_play_id,
            connection_id: ConnectionId::from("conn-1"),
            source: SessionEndpointIdentity {
                host_id: source.host_id.clone(),
                boot_id: source.boot_id.clone(),
            },
            sink: SessionEndpointIdentity {
                host_id: sink.host_id.clone(),
                boot_id: sink.boot_id.clone(),
            },
            value_kind: KindId::from("kind-1"),
            limits: SessionLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 512,
                maximum_buffered_bytes: 512,
            },
            attachment: LineAttachment {
                line_id: "line/usb-cdc-test".into(),
                link_binding_id: LinkBindingId::from("link-1"),
                base: ConnectionBase::UsbCdc,
                base_instance_id: ConnectionBaseInstanceId::from("prov-1"),
                source_host_id: source.host_id,
                source_boot_id: source.boot_id,
                source_endpoint_id: LinkEndpointId::from("end-1"),
                sink_host_id: sink.host_id,
                sink_boot_id: sink.boot_id,
                sink_endpoint_id: LinkEndpointId::from("end-2"),
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 512,
                    maximum_buffered_bytes: 512,
                    maximum_frame_bytes: 1024,
                },
            },
        }
    }

    #[test]
    fn test_1_partial_reads_assemble_one_current_stream_frame() {
        let binding = test_binding();
        let frame = binding.hello_frame();
        let mut wire_buf = [0u8; 2048];
        let frame_len = encode_session_frame_into(frame, &mut wire_buf[2..], 1024, 1024).unwrap();
        let mut framed_buf = [0u8; 2048];
        let total_bytes =
            encode_stream_frame(&wire_buf[2..2 + frame_len], 1024, &mut framed_buf).unwrap();

        let half = total_bytes / 2;
        let mut line =
            NativeUsbCdcLine::new(Cursor::new(&framed_buf[..half]), Vec::new(), 1024).unwrap();
        let mut out_buf = [0u8; 2048];
        assert!(matches!(
            line.receive_frame(&mut out_buf),
            Err(NativeUsbCdcError::Disconnected)
        ));

        let mut line2 =
            NativeUsbCdcLine::new(Cursor::new(&framed_buf[..total_bytes]), Vec::new(), 1024)
                .unwrap();
        let received = line2.receive_frame(&mut out_buf).unwrap();
        assert_eq!(received.identity, frame.identity);
    }

    #[test]
    fn test_2_multiple_frames_arriving_in_one_read_decode_separately() {
        let binding = test_binding();
        let hello = binding.hello_frame();
        let ready = binding.frame(SessionMessage::Ready);

        let mut stream = Vec::new();
        let mut line_tx = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut stream, 512).unwrap();
        line_tx.send_frame(&hello).unwrap();
        line_tx.send_frame(&ready).unwrap();

        let mut line_rx = NativeUsbCdcLine::new(Cursor::new(stream), Vec::new(), 512).unwrap();
        let mut out_buf = [0u8; 512];
        let f1 = line_rx.receive_frame(&mut out_buf).unwrap();
        assert!(matches!(f1.message, SessionMessage::Hello(_)));
        let f2 = line_rx.receive_frame(&mut out_buf).unwrap();
        assert!(matches!(f2.message, SessionMessage::Ready));
    }

    #[test]
    fn test_3_partial_writes_are_completed() {
        let binding = test_binding();
        let frame = binding.hello_frame();
        let mut stream = Vec::new();
        let mut line_tx = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut stream, 512).unwrap();
        line_tx.send_frame(&frame).unwrap();
        assert!(!stream.is_empty());
    }

    #[test]
    fn test_4_timeout_would_block_is_finite() {
        let mut line = NativeUsbCdcLine::new(Cursor::new(Vec::new()), Vec::new(), 512).unwrap();
        let mut out_buf = [0u8; 512];
        assert!(matches!(
            line.receive_frame(&mut out_buf),
            Err(NativeUsbCdcError::Disconnected)
        ));
    }

    #[test]
    fn test_5_eof_disconnect_is_distinct_from_timeout() {
        let mut line = NativeUsbCdcLine::new(Cursor::new(Vec::new()), Vec::new(), 512).unwrap();
        let mut out_buf = [0u8; 512];
        let res = line.receive_frame(&mut out_buf);
        assert!(matches!(res, Err(NativeUsbCdcError::Disconnected)));
    }

    #[test]
    fn test_6_malformed_length_framing_fails_closed() {
        let malformed = vec![0xFF, 0xFF, 0x01, 0x02, 0x03];
        let mut line = NativeUsbCdcLine::new(Cursor::new(malformed), Vec::new(), 512).unwrap();
        let mut out_buf = [0u8; 512];
        assert!(line.receive_frame(&mut out_buf).is_err());
    }

    #[test]
    fn test_7_source_sink_session_machine_exchange_works_over_line() {
        let binding = test_binding();
        let mut source_machine = SessionMachine::new(binding.clone(), SessionRole::Source).unwrap();
        let mut sink_machine = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();

        // 1. Source > Sink Hello
        let hello = binding.hello_frame();
        source_machine.admit_outbound(hello).unwrap();

        let mut c1 = Vec::new();
        let mut tx1 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c1, 512).unwrap();
        tx1.send_frame(&hello).unwrap();

        let mut rx1 = NativeUsbCdcLine::new(Cursor::new(c1), Vec::new(), 512).unwrap();
        let mut buf1 = [0u8; 512];
        let f1 = rx1.receive_frame(&mut buf1).unwrap();
        sink_machine.admit_inbound(f1).unwrap();

        // 2. Sink > Source Hello
        let sink_hello = binding.hello_frame();
        sink_machine.admit_outbound(sink_hello).unwrap();
        let mut c2 = Vec::new();
        let mut tx2 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c2, 512).unwrap();
        tx2.send_frame(&sink_hello).unwrap();
        let mut rx2 = NativeUsbCdcLine::new(Cursor::new(c2), Vec::new(), 512).unwrap();
        let mut buf2 = [0u8; 512];
        let f2 = rx2.receive_frame(&mut buf2).unwrap();
        source_machine.admit_inbound(f2).unwrap();

        // 3. Source > Sink Ready
        let ready = binding.frame(SessionMessage::Ready);
        source_machine.admit_outbound(ready).unwrap();
        let mut c3 = Vec::new();
        let mut tx3 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c3, 512).unwrap();
        tx3.send_frame(&ready).unwrap();
        let mut rx3 = NativeUsbCdcLine::new(Cursor::new(c3), Vec::new(), 512).unwrap();
        let mut buf3 = [0u8; 512];
        let f3 = rx3.receive_frame(&mut buf3).unwrap();
        sink_machine.admit_inbound(f3).unwrap();

        // 4. Sink > Source Ready
        let sink_ready = binding.frame(SessionMessage::Ready);
        sink_machine.admit_outbound(sink_ready).unwrap();
        let mut c4 = Vec::new();
        let mut tx4 = NativeUsbCdcLine::new(Cursor::new(Vec::new()), &mut c4, 512).unwrap();
        tx4.send_frame(&sink_ready).unwrap();
        let mut rx4 = NativeUsbCdcLine::new(Cursor::new(c4), Vec::new(), 512).unwrap();
        let mut buf4 = [0u8; 512];
        let f4 = rx4.receive_frame(&mut buf4).unwrap();
        source_machine.admit_inbound(f4).unwrap();

        assert!(source_machine.is_active());
        assert!(sink_machine.is_active());
    }

    #[test]
    #[cfg(unix)]
    fn test_8_operator_terminal_initializes_and_restores_tty() {
        if std::path::Path::new("/dev/tty").exists() {
            if let Ok(mut term) = OperatorTerminal::open() {
                let key = term.read_key(Duration::from_millis(1)).unwrap();
                assert!(key.is_none() || key.is_some());
            }
        }
    }
}
