//! Host peer for the QEMU FTDI-backed canonical Line session.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::Duration,
};

use conduit_wire::{
    decode_session_frame, encode_session_frame_into, encode_stream_frame, SessionBinding,
    SessionMachine, SessionMessage, SessionRole, WireError,
};

use super::ConduitosError;

const MAXIMUM_FRAME_BYTES: usize = conduitos::usb_line_offer::USB_LINE_MAXIMUM_FRAME_BYTES as usize;
const MAXIMUM_PAYLOAD_BYTES: u32 = conduitos::usb_line_offer::USB_LINE_MAXIMUM_PAYLOAD_BYTES;
const CONNECT_ATTEMPTS: usize = 200;

pub(super) struct Peer {
    stream: UnixStream,
    binding: SessionBinding,
    machine: SessionMachine,
}

pub(super) struct PendingPeer {
    stream: UnixStream,
}

impl PendingPeer {
    pub(super) fn connect(path: &Path) -> Result<Self, ConduitosError> {
        Ok(Self {
            stream: connect(path)?,
        })
    }

    pub(super) fn activate(mut self) -> Result<Peer, ConduitosError> {
        Peer::activate(&mut self.stream)
    }
}

impl Peer {
    fn activate(stream: &mut UnixStream) -> Result<Self, ConduitosError> {
        let mut frame_bytes = [0; MAXIMUM_FRAME_BYTES];
        let length = read_frame(stream, &mut frame_bytes)?;
        let hello = decode_session_frame(
            &frame_bytes[..length],
            MAXIMUM_PAYLOAD_BYTES,
            MAXIMUM_FRAME_BYTES as u32,
        )
        .map_err(wire)?;
        let binding = SessionBinding::from_hello_frame(hello).map_err(wire)?;
        let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink).map_err(wire)?;
        machine.admit_inbound(hello).map_err(wire)?;
        send(
            stream,
            &mut machine,
            &binding,
            binding.hello_frame().message,
        )?;
        receive_expected(stream, &mut machine, &binding, |message| {
            matches!(message, SessionMessage::Ready)
        })?;
        send(stream, &mut machine, &binding, SessionMessage::Ready)?;
        if !machine.is_active() {
            return Err(ConduitosError::refusal(
                "product-journey-usb-line-not-active",
                "canonical Hello/Ready exchange did not activate the peer session",
            ));
        }
        Ok(Self {
            stream: stream.try_clone().map_err(|error| {
                ConduitosError::refusal("product-journey-usb-line-peer-clone", error.to_string())
            })?,
            binding,
            machine,
        })
    }

    pub(super) fn receive_value_and_acknowledge(&mut self) -> Result<(), ConduitosError> {
        receive_expected(
            &mut self.stream,
            &mut self.machine,
            &self.binding,
            |message| {
                matches!(
                    message,
                    SessionMessage::Offered { sequence: 0, payload }
                        if payload == conduitos::product_usb_line::LINE_VALUE
                )
            },
        )?;
        send(
            &mut self.stream,
            &mut self.machine,
            &self.binding,
            SessionMessage::Accepted { sequence: 0 },
        )?;
        send(
            &mut self.stream,
            &mut self.machine,
            &self.binding,
            SessionMessage::Delivered { sequence: 0 },
        )
    }
}

fn connect(path: &Path) -> Result<UnixStream, ConduitosError> {
    let mut last = None;
    for _ in 0..CONNECT_ATTEMPTS {
        match UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(ConduitosError::refusal(
        "product-journey-usb-line-peer-connect",
        last.map_or_else(|| "socket absent".into(), |error| error.to_string()),
    ))
}

fn send(
    stream: &mut UnixStream,
    machine: &mut SessionMachine,
    binding: &SessionBinding,
    message: SessionMessage<'_>,
) -> Result<(), ConduitosError> {
    let frame = binding.frame(message);
    let mut admitted = machine.clone();
    admitted.admit_outbound(frame).map_err(wire)?;
    let mut frame_bytes = [0; MAXIMUM_FRAME_BYTES];
    let length = encode_session_frame_into(
        frame,
        &mut frame_bytes,
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES as u32,
    )
    .map_err(wire)?;
    let mut stream_bytes = [0; MAXIMUM_FRAME_BYTES + 2];
    let stream_length = encode_stream_frame(
        &frame_bytes[..length],
        MAXIMUM_FRAME_BYTES,
        &mut stream_bytes,
    )
    .map_err(|error| {
        ConduitosError::refusal("product-journey-usb-line-frame", format!("{error:?}"))
    })?;
    stream
        .write_all(&stream_bytes[..stream_length])
        .map_err(|error| {
            ConduitosError::refusal("product-journey-usb-line-write", error.to_string())
        })?;
    *machine = admitted;
    Ok(())
}

fn receive_expected(
    stream: &mut UnixStream,
    machine: &mut SessionMachine,
    binding: &SessionBinding,
    matches: impl FnOnce(SessionMessage<'_>) -> bool,
) -> Result<(), ConduitosError> {
    let mut frame_bytes = [0; MAXIMUM_FRAME_BYTES];
    let length = read_frame(stream, &mut frame_bytes)?;
    let frame = decode_session_frame(
        &frame_bytes[..length],
        MAXIMUM_PAYLOAD_BYTES,
        MAXIMUM_FRAME_BYTES as u32,
    )
    .map_err(wire)?;
    if frame.identity != binding.identity() || !matches(frame.message) {
        return Err(ConduitosError::refusal(
            "product-journey-usb-line-message",
            "unexpected canonical session identity or message",
        ));
    }
    machine.admit_inbound(frame).map_err(wire)
}

fn read_frame(
    stream: &mut UnixStream,
    output: &mut [u8; MAXIMUM_FRAME_BYTES],
) -> Result<usize, ConduitosError> {
    let mut header = [0; 2];
    stream.read_exact(&mut header).map_err(|error| {
        ConduitosError::refusal("product-journey-usb-line-read", error.to_string())
    })?;
    let length = usize::from(u16::from_be_bytes(header));
    if length == 0 || length > output.len() {
        return Err(ConduitosError::refusal(
            "product-journey-usb-line-frame-length",
            length.to_string(),
        ));
    }
    stream.read_exact(&mut output[..length]).map_err(|error| {
        ConduitosError::refusal("product-journey-usb-line-read", error.to_string())
    })?;
    Ok(length)
}

fn wire(error: WireError) -> ConduitosError {
    ConduitosError::refusal("product-journey-usb-line-wire", format!("{error:?}"))
}
