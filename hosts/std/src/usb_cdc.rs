//! Native USB CDC ACM carrier implementation for std host.
//!
//! Provides length-prefixed stream framing over any `Read + Write` serial stream or file.

use std::io::{Read, Write};

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
use std::os::unix::io::AsRawFd;

/// Configure raw serial CDC line settings (115200 8N1, raw mode, CLOCAL | CREAD, VMIN, VTIME)
/// on a file or serial port descriptor using native POSIX termios.
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

        // Set raw mode
        libc::cfmakeraw(&mut termios);

        // Set baud rates to 115200
        libc::cfsetispeed(&mut termios, libc::B115200);
        libc::cfsetospeed(&mut termios, libc::B115200);

        // 8 data bits, 1 stop bit, no parity, enable receiver, ignore modem control lines
        termios.c_cflag &= !libc::CSIZE;
        termios.c_cflag |= libc::CS8;
        termios.c_cflag &= !libc::CSTOPB;
        termios.c_cflag &= !libc::PARENB;
        termios.c_cflag |= libc::CLOCAL | libc::CREAD;

        // VMIN and VTIME configuration
        termios.c_cc[libc::VMIN] = min_bytes as libc::cc_t;
        termios.c_cc[libc::VTIME] = timeout_deciseconds as libc::cc_t;

        if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

/// RAII guard that saves prior `/dev/tty` termios state and restores it on Drop.
#[cfg(unix)]
pub struct RawTerminalGuard {
    tty_file: std::fs::File,
    saved_termios: libc::termios,
}

#[cfg(unix)]
impl RawTerminalGuard {
    pub fn new() -> Result<Self, NativeUsbCdcError> {
        let tty_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map_err(NativeUsbCdcError::TtyConfig)?;
        let fd = tty_file.as_raw_fd();
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
                tty_file,
                saved_termios,
            })
        }
    }
}

#[cfg(unix)]
impl Drop for RawTerminalGuard {
    fn drop(&mut self) {
        let fd = self.tty_file.as_raw_fd();
        unsafe {
            let _ = libc::tcsetattr(fd, libc::TCSANOW, &self.saved_termios);
        }
    }
}

pub struct NativeUsbCdcCarrier<R, W> {
    reader: R,
    writer: W,
    maximum_frame_bytes: usize,
    decoder: StreamFrameDecoder<2048>,
}

impl<R: Read, W: Write> NativeUsbCdcCarrier<R, W> {
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

    pub fn maximum_frame_bytes(&self) -> usize {
        self.maximum_frame_bytes
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
                Ok(0) => return Err(NativeUsbCdcError::WouldBlock),
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
        ActivePlayId, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
        FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits,
        PlanId,
    };
    use conduit_wire::{SessionBinding, SessionMessage};
    use std::io::Cursor;

    #[test]
    fn usb_cdc_carrier_round_trips_framed_session_over_chunked_stream() {
        let binding = SessionBinding {
            protocol_version: 1,
            plan_id: PlanId::from("plan-1"),
            source_fragment_id: FragmentId::from("frag-1"),
            sink_fragment_id: FragmentId::from("frag-2"),
            source_active_play_id: ActivePlayId::from("play-1"),
            sink_active_play_id: ActivePlayId::from("play-2"),
            connection_id: ConnectionId::from("conn-1"),
            link_binding_id: LinkBindingId::from("link-1"),
            provider: ConnectionProvider::UsbCdc,
            provider_instance_id: ConnectionProviderInstanceId::from("prov-1"),
            source: LinkEndpoint {
                host_id: HostId::from("host-1"),
                boot_id: BootId::from("boot-1"),
                endpoint_id: LinkEndpointId::from("end-1"),
            },
            sink: LinkEndpoint {
                host_id: HostId::from("host-2"),
                boot_id: BootId::from("boot-2"),
                endpoint_id: LinkEndpointId::from("end-2"),
            },
            value_kind: KindId::from("kind-1"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 512,
            },
        };

        let frame = SessionFrame {
            identity: binding.identity(),
            message: SessionMessage::Ready,
        };

        let mut read_buf = Vec::new();
        let mut carrier_tx =
            NativeUsbCdcCarrier::new(Cursor::new(Vec::new()), &mut read_buf, 512).unwrap();
        carrier_tx.send_frame(&frame).unwrap();

        let mut carrier_rx =
            NativeUsbCdcCarrier::new(Cursor::new(read_buf), Vec::new(), 512).unwrap();
        let mut frame_buf = [0u8; 512];
        let received = carrier_rx.receive_frame(&mut frame_buf).unwrap();
        assert_eq!(received.identity, frame.identity);
    }
}
