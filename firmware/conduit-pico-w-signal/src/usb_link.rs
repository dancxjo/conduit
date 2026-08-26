//! USB CDC ACM link transport for Conduit SessionFrames.
//!
//! Handles length-prefixed stream framing over CDC 0 USB packets.

use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionFrame};
use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::CdcAcmClass;

use super::usb::PicoUsbCdcLine;
pub use crate::remote_error::RemoteError as UsbLinkError;

pub struct UsbLinkSession {
    class: CdcAcmClass<'static, usb::Driver<'static, USB>>,
    decoder: StreamFrameDecoder<4096>,
}

impl UsbLinkSession {
    pub fn new(line: PicoUsbCdcLine) -> Result<Self, UsbLinkError> {
        Ok(Self {
            class: line.class,
            decoder: StreamFrameDecoder::new(4096).map_err(UsbLinkError::Framing)?,
        })
    }

    /// Wait for USB host connection on CDC 0 interface.
    pub async fn wait_connection(&mut self) {
        self.class.wait_connection().await;
    }

    /// Discard incomplete or invalid bytes at an explicit CDC control-session
    /// boundary. The buffer remains fixed; this only restores decoder state.
    pub fn reset_stream_decoder(&mut self) {
        self.decoder =
            StreamFrameDecoder::new(4096).expect("the fixed USB stream decoder limit is valid");
    }

    /// Receive the next framed SessionFrame from the USB CDC ACM link.
    #[allow(dead_code)]
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
                let frame = decode_session_frame(&frame_buf[..frame_bytes.len()], 1024, 1024)?;
                return Ok(frame);
            }

            let read_bytes = self
                .class
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
    #[allow(dead_code)]
    pub async fn send_frame(&mut self, frame: &SessionFrame<'_>) -> Result<(), UsbLinkError> {
        let mut wire_buf = [0u8; 2048];
        let frame_len = encode_session_frame_into(*frame, &mut wire_buf[2..], 1024, 1024)?;
        let mut framed_buf = [0u8; 2048];
        let total_bytes = encode_stream_frame(&wire_buf[2..2 + frame_len], 1024, &mut framed_buf)?;

        let mut offset = 0;
        while offset < total_bytes {
            let chunk_size = (total_bytes - offset).min(64);
            self.class
                .write_packet(&framed_buf[offset..offset + chunk_size])
                .await
                .map_err(|_| UsbLinkError::UsbDisconnected)?;
            offset += chunk_size;
        }

        if total_bytes > 0 && total_bytes % 64 == 0 {
            let _ = self.class.write_packet(&[]).await;
        }

        Ok(())
    }

    /// Receive next raw length-prefixed stream frame payload without SessionFrame decoding.
    #[allow(
        dead_code,
        reason = "exclusive firmware modes select one raw receive policy"
    )]
    pub async fn receive_raw_stream_frame<'a>(
        &mut self,
        buf: &'a mut [u8],
    ) -> Result<&'a [u8], UsbLinkError> {
        let mut packet_buf = [0u8; 64];
        loop {
            if let Some(frame_bytes) = self.decoder.next_frame()? {
                if buf.len() < frame_bytes.len() {
                    return Err(UsbLinkError::BufferOverflow);
                }
                buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                return Ok(&buf[..frame_bytes.len()]);
            }

            let read_bytes = self
                .class
                .read_packet(&mut packet_buf)
                .await
                .map_err(|_| UsbLinkError::UsbDisconnected)?;

            if read_bytes == 0 {
                continue;
            }

            self.decoder.accept_bytes(&packet_buf[..read_bytes])?;
        }
    }

    /// Receive developer lifecycle control while recovering from unrelated
    /// CDC startup bytes. Ordinary Session traffic deliberately does not use
    /// this timeout-and-resynchronize policy.
    pub async fn receive_control_stream_frame<'a>(
        &mut self,
        buf: &'a mut [u8],
    ) -> Result<&'a [u8], UsbLinkError> {
        let mut packet_buf = [0u8; 64];
        loop {
            match self.decoder.next_frame() {
                Ok(Some(frame_bytes)) => {
                    if buf.len() < frame_bytes.len() {
                        self.reset_stream_decoder();
                        return Err(UsbLinkError::BufferOverflow);
                    }
                    buf[..frame_bytes.len()].copy_from_slice(frame_bytes);
                    return Ok(&buf[..frame_bytes.len()]);
                }
                Ok(None) => {}
                Err(_) => {
                    self.reset_stream_decoder();
                }
            }

            let read_bytes = match with_timeout(
                Duration::from_millis(500),
                self.class.read_packet(&mut packet_buf),
            )
            .await
            {
                Ok(Ok(read_bytes)) => read_bytes,
                Ok(Err(_)) => return Err(UsbLinkError::UsbDisconnected),
                Err(_) => {
                    self.reset_stream_decoder();
                    continue;
                }
            };
            if read_bytes != 0 {
                if self
                    .decoder
                    .accept_bytes(&packet_buf[..read_bytes])
                    .is_err()
                {
                    self.reset_stream_decoder();
                }
            }
        }
    }

    /// Send raw stream frame payload without SessionFrame encoding.
    pub async fn send_raw_stream_frame(&mut self, payload: &[u8]) -> Result<(), UsbLinkError> {
        let mut framed_buf = [0u8; 4098];
        let total_bytes = encode_stream_frame(payload, 4096, &mut framed_buf)?;

        let mut offset = 0;
        while offset < total_bytes {
            let chunk_size = (total_bytes - offset).min(64);
            self.class
                .write_packet(&framed_buf[offset..offset + chunk_size])
                .await
                .map_err(|_| UsbLinkError::UsbDisconnected)?;
            offset += chunk_size;
        }

        if total_bytes > 0 && total_bytes % 64 == 0 {
            let _ = self.class.write_packet(&[]).await;
        }

        Ok(())
    }
}
