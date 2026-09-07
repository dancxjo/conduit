//! Canonical bounded session frames carried through the exact FTDI byte stream.

use conduit_wire::{
    SessionBinding, SessionFrame, SessionMachine, SessionMessage, SessionRole,
    SessionTerminalDisposition, StreamFrameError, WireError, decode_session_frame,
    encode_session_frame_into, encode_stream_frame,
};

use crate::{
    arch::{
        FTDI_PACKET_BYTES, FTDI_PAYLOAD_BYTES, FtdiLineError, FtdiLineSession, UsbDevice, XhciReady,
    },
    usb_line_offer::{
        AdmittedUsbLineBasis, USB_LINE_MAXIMUM_FRAME_BYTES, USB_LINE_MAXIMUM_PAYLOAD_BYTES,
        UsbLineObservation, UsbLineOfferError,
    },
};

pub const USB_LINE_STREAM_BYTES: usize = USB_LINE_MAXIMUM_FRAME_BYTES as usize + 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbLineSessionError {
    Carrier(FtdiLineError),
    Framing(StreamFrameError),
    Wire(WireError),
    Current(UsbLineOfferError),
    IncompleteFrame,
    PayloadStorageExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceivedSessionMessage {
    Hello,
    Ready,
    Offered {
        sequence: u64,
        payload: [u8; USB_LINE_MAXIMUM_PAYLOAD_BYTES as usize],
        length: u8,
    },
    Pressure(u64),
    Accepted(u64),
    Delivered(u64),
    InputClosed(u64),
    Cancelled(u16),
    Failed(u16),
    Terminal {
        disposition: SessionTerminalDisposition,
        final_sequence: u64,
    },
}

pub struct UsbLineSession {
    machine: SessionMachine,
    outgoing_frame: [u8; USB_LINE_MAXIMUM_FRAME_BYTES as usize],
    outgoing_stream: [u8; USB_LINE_STREAM_BYTES],
    incoming_stream: [u8; USB_LINE_STREAM_BYTES],
    incoming_length: usize,
}

impl UsbLineSession {
    pub fn new(binding: SessionBinding, role: SessionRole) -> Result<Self, UsbLineSessionError> {
        Ok(Self {
            machine: SessionMachine::new(binding, role).map_err(UsbLineSessionError::Wire)?,
            outgoing_frame: [0; USB_LINE_MAXIMUM_FRAME_BYTES as usize],
            outgoing_stream: [0; USB_LINE_STREAM_BYTES],
            incoming_stream: [0; USB_LINE_STREAM_BYTES],
            incoming_length: 0,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        self.machine.binding()
    }

    pub fn is_active(&self) -> bool {
        self.machine.is_active()
    }

    pub fn send(
        &mut self,
        carrier: &mut FtdiLineSession,
        controller: &mut XhciReady,
        device: &UsbDevice,
        basis: &AdmittedUsbLineBasis,
        observation: &UsbLineObservation,
        message: SessionMessage<'_>,
    ) -> Result<(), UsbLineSessionError> {
        basis
            .validate_current(observation)
            .map_err(UsbLineSessionError::Current)?;
        let binding = self.machine.binding().clone();
        let frame = SessionFrame {
            identity: binding.identity(),
            message,
        };
        let mut admitted = self.machine.clone();
        admitted
            .admit_outbound(frame)
            .map_err(UsbLineSessionError::Wire)?;
        let frame_length = encode_session_frame_into(
            frame,
            &mut self.outgoing_frame,
            USB_LINE_MAXIMUM_PAYLOAD_BYTES,
            USB_LINE_MAXIMUM_FRAME_BYTES,
        )
        .map_err(UsbLineSessionError::Wire)?;
        let stream_length = encode_stream_frame(
            &self.outgoing_frame[..frame_length],
            USB_LINE_MAXIMUM_FRAME_BYTES as usize,
            &mut self.outgoing_stream,
        )
        .map_err(UsbLineSessionError::Framing)?;
        for packet in self.outgoing_stream[..stream_length].chunks(FTDI_PACKET_BYTES) {
            basis
                .validate_current(observation)
                .map_err(UsbLineSessionError::Current)?;
            carrier
                .send(controller, device, packet)
                .map_err(UsbLineSessionError::Carrier)?;
        }
        self.machine = admitted;
        Ok(())
    }

    pub fn receive(
        &mut self,
        carrier: &mut FtdiLineSession,
        controller: &mut XhciReady,
        device: &UsbDevice,
        basis: &AdmittedUsbLineBasis,
        observation: &UsbLineObservation,
    ) -> Result<ReceivedSessionMessage, UsbLineSessionError> {
        loop {
            basis
                .validate_current(observation)
                .map_err(UsbLineSessionError::Current)?;
            if let Some(frame_length) =
                complete_frame_length(&self.incoming_stream, self.incoming_length)?
            {
                let frame = decode_session_frame(
                    &self.incoming_stream[2..2 + frame_length],
                    USB_LINE_MAXIMUM_PAYLOAD_BYTES,
                    USB_LINE_MAXIMUM_FRAME_BYTES,
                )
                .map_err(UsbLineSessionError::Wire)?;
                self.machine
                    .admit_inbound(frame)
                    .map_err(UsbLineSessionError::Wire)?;
                let received = ReceivedSessionMessage::copy_from(frame.message)?;
                let consumed = frame_length + 2;
                self.incoming_stream
                    .copy_within(consumed..self.incoming_length, 0);
                self.incoming_length -= consumed;
                return Ok(received);
            }
            let mut packet = [0; FTDI_PAYLOAD_BYTES];
            let received = carrier
                .receive(controller, device, &mut packet)
                .map_err(UsbLineSessionError::Carrier)?;
            if self.incoming_length + received > self.incoming_stream.len() {
                return Err(UsbLineSessionError::Framing(
                    StreamFrameError::BufferOverflow,
                ));
            }
            self.incoming_stream[self.incoming_length..self.incoming_length + received]
                .copy_from_slice(&packet[..received]);
            self.incoming_length += received;
        }
    }

    pub fn refuse_incomplete_after_loss(&self) -> Result<(), UsbLineSessionError> {
        if self.incoming_length == 0 {
            Ok(())
        } else {
            Err(UsbLineSessionError::IncompleteFrame)
        }
    }
}

impl ReceivedSessionMessage {
    fn copy_from(message: SessionMessage<'_>) -> Result<Self, UsbLineSessionError> {
        Ok(match message {
            SessionMessage::Hello(_) => Self::Hello,
            SessionMessage::Ready => Self::Ready,
            SessionMessage::Offered { sequence, payload } => {
                let length = u8::try_from(payload.len())
                    .map_err(|_| UsbLineSessionError::PayloadStorageExceeded)?;
                let mut copy = [0; USB_LINE_MAXIMUM_PAYLOAD_BYTES as usize];
                copy[..payload.len()].copy_from_slice(payload);
                Self::Offered {
                    sequence,
                    payload: copy,
                    length,
                }
            }
            SessionMessage::Pressure { sequence } => Self::Pressure(sequence),
            SessionMessage::Accepted { sequence } => Self::Accepted(sequence),
            SessionMessage::Delivered { sequence } => Self::Delivered(sequence),
            SessionMessage::InputClosed { final_sequence } => Self::InputClosed(final_sequence),
            SessionMessage::Cancelled { code } => Self::Cancelled(code),
            SessionMessage::Failed { code } => Self::Failed(code),
            SessionMessage::Terminal {
                disposition,
                final_sequence,
            } => Self::Terminal {
                disposition,
                final_sequence,
            },
        })
    }
}

fn complete_frame_length(
    stream: &[u8; USB_LINE_STREAM_BYTES],
    available: usize,
) -> Result<Option<usize>, UsbLineSessionError> {
    if available < 2 {
        return Ok(None);
    }
    let length = usize::from(u16::from_be_bytes([stream[0], stream[1]]));
    if length == 0 {
        return Err(UsbLineSessionError::Framing(
            StreamFrameError::ZeroLengthFrame,
        ));
    }
    if length > USB_LINE_MAXIMUM_FRAME_BYTES as usize {
        return Err(UsbLineSessionError::Framing(
            StreamFrameError::FrameExceedsLimit {
                length,
                limit: USB_LINE_MAXIMUM_FRAME_BYTES as usize,
            },
        ));
    }
    Ok((available >= length + 2).then_some(length))
}

#[cfg(test)]
#[path = "usb_line_session_tests.rs"]
mod tests;
