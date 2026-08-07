//! Native USB CDC ACM carrier implementation for std host.
//!
//! Provides length-prefixed stream framing over any `Read + Write` serial stream or file.

use std::io::{Read, Write};

use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder, StreamFrameError};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionFrame, WireError,
};

#[derive(Debug)]
pub enum NativeUsbCdcError {
    InvalidLimit,
    Read(std::io::ErrorKind),
    Write(std::io::ErrorKind),
    Framing(StreamFrameError),
    Codec(WireError),
    Disconnected,
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

pub struct NativeUsbCdcCarrier<R, W> {
    reader: R,
    writer: W,
    maximum_frame_bytes: usize,
    decoder: StreamFrameDecoder<2048>,
}

impl<R: Read, W: Write> NativeUsbCdcCarrier<R, W> {
    pub fn new(reader: R, writer: W, maximum_frame_bytes: usize) -> Result<Self, NativeUsbCdcError> {
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
        let mut wire_buf = [0u8; 1024];
        let frame_len = encode_session_frame_into(
            *frame,
            &mut wire_buf[2..],
            self.maximum_frame_bytes as u32,
            self.maximum_frame_bytes as u32,
        )?;
        let mut framed_buf = [0u8; 1026];
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

            let read_bytes = self
                .reader
                .read(&mut chunk)
                .map_err(|e| NativeUsbCdcError::Read(e.kind()))?;
            if read_bytes == 0 {
                return Err(NativeUsbCdcError::Disconnected);
            }
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
