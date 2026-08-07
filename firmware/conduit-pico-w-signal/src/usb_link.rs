//! USB CDC ACM link transport for Conduit SessionFrames.
//!
//! Handles length-prefixed stream framing over CDC 0 USB packets.

use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder, StreamFrameError};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionFrame, WireError,
};
use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::{Receiver, Sender};

use super::usb::PicoUsbCdcCarrier;

#[derive(Debug)]
pub enum UsbLinkError {
    UsbDisconnected,
    Framing(StreamFrameError),
    Codec(WireError),
    BufferOverflow,
}

impl From<StreamFrameError> for UsbLinkError {
    fn from(err: StreamFrameError) -> Self {
        Self::Framing(err)
    }
}

impl From<WireError> for UsbLinkError {
    fn from(err: WireError) -> Self {
        Self::Codec(err)
    }
}

pub struct UsbLinkSession {
    sender: Sender<'static, usb::Driver<'static, USB>>,
    receiver: Receiver<'static, usb::Driver<'static, USB>>,
    decoder: StreamFrameDecoder<1024>,
}

impl UsbLinkSession {
    pub fn new(carrier: PicoUsbCdcCarrier) -> Result<Self, UsbLinkError> {
        Ok(Self {
            sender: carrier.sender,
            receiver: carrier.receiver,
            decoder: StreamFrameDecoder::new(1024).map_err(UsbLinkError::Framing)?,
        })
    }

    /// Receive the next framed SessionFrame from the USB CDC ACM link.
    pub async fn receive_frame<'a>(
        &mut self,
        frame_buf: &'a mut [u8],
    ) -> Result<SessionFrame<'a>, UsbLinkError> {
        let mut packet_buf = [0u8; 64];
        loop {
            if let Some(frame_bytes) = self.decoder.next_frame()? {
                if frame_buf.len() < frame_bytes.len() {
                    return Err(UsbLinkError::BufferOverflow);
                }
                frame_buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                let frame = decode_session_frame(&frame_buf[..frame_bytes.len()], 512, 512)?;
                return Ok(frame);
            }

            self.receiver.wait_connection().await;
            let read_bytes = self
                .receiver
                .read_packet(&mut packet_buf)
                .await
                .map_err(|_| UsbLinkError::UsbDisconnected)?;

            if read_bytes == 0 {
                continue;
            }

            self.decoder.accept_bytes(&packet_buf[..read_bytes])?;
        }
    }

    /// Send a SessionFrame over the USB CDC ACM link using length-prefixed framing.
    pub async fn send_frame(&mut self, frame: &SessionFrame<'_>) -> Result<(), UsbLinkError> {
        let mut wire_buf = [0u8; 512];
        let frame_len = encode_session_frame_into(*frame, &mut wire_buf[2..], 512, 512)?;
        let mut framed_buf = [0u8; 514];
        let total_bytes = encode_stream_frame(&wire_buf[2..2 + frame_len], 512, &mut framed_buf)?;

        self.sender.wait_connection().await;

        let mut offset = 0;
        while offset < total_bytes {
            let chunk_size = (total_bytes - offset).min(64);
            self.sender
                .write_packet(&framed_buf[offset..offset + chunk_size])
                .await
                .map_err(|_| UsbLinkError::UsbDisconnected)?;
            offset += chunk_size;
        }

        if total_bytes > 0 && total_bytes % 64 == 0 {
            let _ = self.sender.write_packet(&[]).await;
        }

        Ok(())
    }
}
