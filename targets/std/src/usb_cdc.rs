//! Native USB CDC ACM line implementation for std host.
//!
//! Provides length-prefixed stream framing over native POSIX serial endpoints
//! and generic `Read + Write` streams.

use std::io::{Read, Write};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
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

    /// Discard bytes already queued by a device before a framed control
    /// exchange begins. This is intentionally bounded and non-blocking; it is
    /// used for devices whose first CDC connection emits a finite diagnostic
    /// transcript before switching to length-prefixed service frames.
    pub fn discard_pending_raw_bytes(&mut self) -> Result<usize, NativeUsbCdcError> {
        const MAXIMUM_DISCARD_BYTES: usize = 4096;
        const QUIET_MILLIS: libc::c_int = 100;
        const MAXIMUM_WAIT: Duration = Duration::from_secs(2);
        let mut discarded = 0_usize;
        let mut chunk = [0_u8; 64];
        let deadline = Instant::now() + MAXIMUM_WAIT;
        loop {
            let mut pfd = libc::pollfd {
                fd: self.fd.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let now = Instant::now();
            if now >= deadline {
                return Ok(discarded);
            }
            let remaining_millis = (deadline - now).as_millis().min(QUIET_MILLIS as u128);
            let ready = unsafe { libc::poll(&mut pfd, 1, remaining_millis as libc::c_int) };
            if ready < 0 {
                return Err(NativeUsbCdcError::Read(
                    std::io::Error::last_os_error().kind(),
                ));
            }
            if ready == 0 {
                return Ok(discarded);
            }
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let remaining = MAXIMUM_DISCARD_BYTES.saturating_sub(discarded);
            if remaining == 0 {
                return Err(NativeUsbCdcError::InvalidLimit);
            }
            let length = remaining.min(chunk.len());
            let read = unsafe { libc::read(self.fd.0, chunk.as_mut_ptr() as *mut _, length) };
            if read > 0 {
                discarded += read as usize;
            } else if read == 0 {
                return Err(NativeUsbCdcError::Disconnected);
            } else {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(discarded);
                }
                return Err(NativeUsbCdcError::Read(error.kind()));
            }
        }
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
mod tests;
