//! USB CDC ACM link transport for Conduit SessionFrames.
//!
//! Handles length-prefixed stream framing over CDC 0 USB packets.

use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder, StreamFrameError};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionFrame, WireError,
};
use embassy_rp::peripherals::USB;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::CdcAcmClass;

use super::usb::PicoUsbCdcCarrier;
use crate::receipts::UsbClueError;

#[allow(dead_code)]
pub type UsbLinkResult<T> = Result<T, UsbLinkError>;

#[allow(dead_code)]
#[derive(Debug)]
pub enum UsbLinkError {
    UsbDisconnected,
    Framing(StreamFrameError),
    Codec(WireError),
    Clue(UsbClueError),
    BufferOverflow,
    InvalidGeneratedEndpoint,
    InvalidSignal,
    Storage(conduit_kernel::StorageError),
    Kernel(conduit_kernel::scheduler::SchedulerError),
    ClueStorage(conduit_kernel::ClueError),
    KernelIdle,
    KernelCompletedEarly,
    KernelCancelled,
    KernelTerminalInvariant,
}

impl UsbLinkError {
    #[allow(dead_code)]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UsbDisconnected => "usb-disconnected",
            Self::Framing(_) => "malformed-stream-frame",
            Self::Codec(_) => "invalid-session-frame",
            Self::Clue(_) => "clue-channel-failure",
            Self::BufferOverflow => "bounded-buffer-overflow",
            Self::InvalidGeneratedEndpoint => "invalid-generated-endpoint",
            Self::InvalidSignal => "invalid-signal",
            Self::Storage(_) => "kernel-storage-failure",
            Self::Kernel(_) => "kernel-scheduler-failure",
            Self::ClueStorage(_) => "kernel-clue-failure",
            Self::KernelIdle => "kernel-idle-before-effect",
            Self::KernelCompletedEarly => "kernel-completed-before-effect",
            Self::KernelCancelled => "kernel-cancelled",
            Self::KernelTerminalInvariant => "kernel-terminal-invariant",
        }
    }
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

impl From<UsbClueError> for UsbLinkError {
    fn from(err: UsbClueError) -> Self {
        Self::Clue(err)
    }
}

pub struct UsbLinkSession {
    class: CdcAcmClass<'static, usb::Driver<'static, USB>>,
    decoder: StreamFrameDecoder<1024>,
}

impl UsbLinkSession {
    pub fn new(carrier: PicoUsbCdcCarrier) -> Result<Self, UsbLinkError> {
        Ok(Self {
            class: carrier.class,
            decoder: StreamFrameDecoder::new(1024).map_err(UsbLinkError::Framing)?,
        })
    }

    /// Wait for USB host connection on CDC 0 interface.
    pub async fn wait_connection(&mut self) {
        self.class.wait_connection().await;
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

    /// Send raw stream frame payload without SessionFrame encoding.
    pub async fn send_raw_stream_frame(&mut self, payload: &[u8]) -> Result<(), UsbLinkError> {
        let mut framed_buf = [0u8; 1024];
        let total_bytes = encode_stream_frame(payload, 1024, &mut framed_buf)?;

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
