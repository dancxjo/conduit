//! Bounded foreground terminal effect over the owner's selected Unix provider.
//! The client must write and flush the exact bytes before returning an ack;
//! a timeout, bad digest, or EOF never produces a terminal effect receipt.
use crate::terminal_face_mask::MAX_TERMINAL_DOCUMENT_BYTES;
use conduit_presentation::MaskShow;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

const FRAME: &[u8; 8] = b"CDTFRAME";
const ACK: &[u8; 8] = b"CDTACK01";
const EXCHANGE_MILLIS: u64 = 5_000;
const HEADER_BYTES: usize = 8 + 32 + 4 + 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFrameReceipt {
    pub bytes_written: u32,
    pub frame_sha256: [u8; 32],
    pub show_sha256: [u8; 32],
}

pub(super) struct AttachedTerminalWriter<'a> {
    stream: &'a mut UnixStream,
    show_sha256: [u8; 32],
    sent: Option<TerminalFrameReceipt>,
    acknowledged: bool,
    deadline: Instant,
}

impl<'a> AttachedTerminalWriter<'a> {
    pub(super) fn new(stream: &'a mut UnixStream, show: &MaskShow) -> Self {
        Self {
            stream,
            show_sha256: Sha256::digest(show.show_id.as_str().as_bytes()).into(),
            sent: None,
            acknowledged: false,
            deadline: Instant::now() + Duration::from_millis(EXCHANGE_MILLIS),
        }
    }
}

impl Write for AttachedTerminalWriter<'_> {
    fn write(&mut self, frame: &[u8]) -> io::Result<usize> {
        if self.sent.is_some() || frame.is_empty() || frame.len() > MAX_TERMINAL_DOCUMENT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "terminal frame is empty, repeated, or over the admitted bound",
            ));
        }
        let receipt = TerminalFrameReceipt {
            bytes_written: frame.len() as u32,
            frame_sha256: Sha256::digest(frame).into(),
            show_sha256: self.show_sha256,
        };
        let header = header(FRAME, &receipt);
        write_all_until(self.stream, &header, self.deadline)?;
        write_all_until(self.stream, frame, self.deadline)?;
        self.sent = Some(receipt);
        Ok(frame.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.acknowledged {
            return Ok(());
        }
        let expected = self.sent.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "terminal frame was not sent")
        })?;
        let mut ack = [0; HEADER_BYTES];
        read_exact_until(self.stream, &mut ack, self.deadline)?;
        if parse_header(&ack, ACK)? != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "terminal output acknowledgement differs from the exact frame",
            ));
        }
        self.acknowledged = true;
        Ok(())
    }
}

/// Foreground provider effect. A caller may report these bytes and digest as
/// written+flushed only after this function succeeds; it cannot mint a Show.
pub fn receive_terminal_frame_and_ack(
    stream: &mut UnixStream,
    output: &mut impl Write,
) -> io::Result<TerminalFrameReceipt> {
    let deadline = Instant::now() + Duration::from_millis(EXCHANGE_MILLIS);
    let mut request = [0; HEADER_BYTES];
    read_exact_until(stream, &mut request, deadline)?;
    let receipt = parse_header(&request, FRAME)?;
    let length = usize::try_from(receipt.bytes_written).map_err(|_| pressure())?;
    if length == 0 || length > MAX_TERMINAL_DOCUMENT_BYTES {
        return Err(pressure());
    }
    let mut bytes = vec![0; length];
    read_exact_until(stream, &mut bytes, deadline)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != receipt.frame_sha256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal frame digest differs",
        ));
    }
    output.write_all(&bytes)?;
    output.flush()?;
    write_all_until(stream, &header(ACK, &receipt), deadline)?;
    Ok(receipt)
}

fn header(magic: &[u8; 8], receipt: &TerminalFrameReceipt) -> [u8; HEADER_BYTES] {
    let mut bytes = [0; HEADER_BYTES];
    bytes[..8].copy_from_slice(magic);
    bytes[8..40].copy_from_slice(&receipt.show_sha256);
    bytes[40..44].copy_from_slice(&receipt.bytes_written.to_le_bytes());
    bytes[44..76].copy_from_slice(&receipt.frame_sha256);
    bytes
}

fn parse_header(bytes: &[u8; HEADER_BYTES], magic: &[u8; 8]) -> io::Result<TerminalFrameReceipt> {
    if &bytes[..8] != magic {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal effect protocol differs",
        ));
    }
    Ok(TerminalFrameReceipt {
        show_sha256: bytes[8..40].try_into().expect("fixed header"),
        bytes_written: u32::from_le_bytes(bytes[40..44].try_into().expect("fixed header")),
        frame_sha256: bytes[44..76].try_into().expect("fixed header"),
    })
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "terminal effect deadline expired",
        ));
    }
    Ok(left)
}

fn read_exact_until(
    stream: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        match stream.read(bytes) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn write_all_until(stream: &mut UnixStream, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        match stream.write(bytes) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn pressure() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "terminal frame exceeds admitted bound",
    )
}
