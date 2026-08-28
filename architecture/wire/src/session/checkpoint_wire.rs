//! Fixed-buffer exchange of finite session checkpoints on a replacement Line.

use conduit_core::PROTOCOL_VERSION;

use super::{
    SessionCheckpoint, SessionCheckpointOffer, SessionIdentity, SessionLimits,
    SessionTransferCheckpoint,
};
use crate::{WireError, MAX_ID_BYTES};

const CHECKPOINT_MAGIC: [u8; 4] = *b"CNDC";
const CHECKPOINT_WIRE_VERSION: u8 = 1;

pub fn encode_session_checkpoint_into(
    offer: SessionCheckpointOffer<'_>,
    output: &mut [u8],
    maximum_frame_bytes: u32,
) -> Result<usize, WireError> {
    if offer.identity.protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let maximum = usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)?;
    let mut writer = Writer::new(output, maximum)?;
    writer.bytes(&CHECKPOINT_MAGIC)?;
    writer.u8(CHECKPOINT_WIRE_VERSION)?;
    writer.u16(offer.identity.protocol_version)?;
    for identity in identity_fields(offer.identity) {
        writer.text(identity)?;
    }
    writer.u16(offer.identity.limits.maximum_in_flight_items)?;
    writer.u32(offer.identity.limits.maximum_payload_bytes)?;
    writer.u32(offer.identity.limits.maximum_buffered_bytes)?;
    writer.u64(offer.checkpoint.next_sequence)?;
    match offer.checkpoint.transfer {
        SessionTransferCheckpoint::None => writer.u8(0)?,
        SessionTransferCheckpoint::Offered(sequence) => {
            writer.u8(1)?;
            writer.u64(sequence)?;
        }
        SessionTransferCheckpoint::Accepted(sequence) => {
            writer.u8(2)?;
            writer.u64(sequence)?;
        }
    }
    writer.u8(u8::from(offer.checkpoint.input_closed))?;
    Ok(writer.len())
}

pub fn decode_session_checkpoint(
    frame: &[u8],
    maximum_frame_bytes: u32,
) -> Result<SessionCheckpointOffer<'_>, WireError> {
    if frame.len() > usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)? {
        return Err(WireError::OversizedFrame);
    }
    let mut cursor = Cursor::new(frame);
    if cursor.take(4)? != CHECKPOINT_MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if cursor.u8()? != CHECKPOINT_WIRE_VERSION {
        return Err(WireError::UnsupportedWireFormat);
    }
    let protocol_version = cursor.u16()?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let identity = SessionIdentity {
        protocol_version,
        plan_id: cursor.text()?,
        source_fragment_id: cursor.text()?,
        sink_fragment_id: cursor.text()?,
        source_active_play_id: cursor.text()?,
        sink_active_play_id: cursor.text()?,
        connection_id: cursor.text()?,
        source_host_id: cursor.text()?,
        source_boot_id: cursor.text()?,
        sink_host_id: cursor.text()?,
        sink_boot_id: cursor.text()?,
        value_kind: cursor.text()?,
        limits: SessionLimits {
            maximum_in_flight_items: cursor.u16()?,
            maximum_payload_bytes: cursor.u32()?,
            maximum_buffered_bytes: cursor.u32()?,
        },
    };
    let next_sequence = cursor.u64()?;
    let transfer = match cursor.u8()? {
        0 => SessionTransferCheckpoint::None,
        1 => SessionTransferCheckpoint::Offered(cursor.u64()?),
        2 => SessionTransferCheckpoint::Accepted(cursor.u64()?),
        _ => return Err(WireError::InvalidState),
    };
    let input_closed = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return Err(WireError::InvalidState),
    };
    if !cursor.is_empty() {
        return Err(WireError::TrailingGarbage);
    }
    Ok(SessionCheckpointOffer {
        identity,
        checkpoint: SessionCheckpoint {
            next_sequence,
            transfer,
            input_closed,
        },
    })
}

fn identity_fields(identity: SessionIdentity<'_>) -> [&str; 11] {
    [
        identity.plan_id,
        identity.source_fragment_id,
        identity.sink_fragment_id,
        identity.source_active_play_id,
        identity.sink_active_play_id,
        identity.connection_id,
        identity.source_host_id,
        identity.source_boot_id,
        identity.sink_host_id,
        identity.sink_boot_id,
        identity.value_kind,
    ]
}

struct Writer<'a> {
    output: &'a mut [u8],
    limit: usize,
    offset: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8], limit: usize) -> Result<Self, WireError> {
        if limit == 0 || output.len() < limit {
            return Err(WireError::OutputTooSmall);
        }
        Ok(Self {
            output,
            limit,
            offset: 0,
        })
    }

    fn len(&self) -> usize {
        self.offset
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), WireError> {
        let end = self
            .offset
            .checked_add(value.len())
            .filter(|end| *end <= self.limit)
            .ok_or(WireError::OversizedFrame)?;
        self.output[self.offset..end].copy_from_slice(value);
        self.offset = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), WireError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), WireError> {
        self.bytes(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), WireError> {
        if value.len() > MAX_ID_BYTES || value.len() > usize::from(u16::MAX) {
            return Err(WireError::IdentifierTooLong);
        }
        self.u16(value.len() as u16)?;
        self.bytes(value.as_bytes())
    }
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(frame: &'a [u8]) -> Self {
        Self { remaining: frame }
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WireError> {
        if self.remaining.len() < length {
            return Err(WireError::TruncatedFrame);
        }
        let (value, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, WireError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn text(&mut self) -> Result<&'a str, WireError> {
        let length = usize::from(self.u16()?);
        if length > MAX_ID_BYTES {
            return Err(WireError::IdentifierTooLong);
        }
        core::str::from_utf8(self.take(length)?).map_err(|_| WireError::InvalidIdentifierEncoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer() -> SessionCheckpointOffer<'static> {
        SessionCheckpointOffer {
            identity: SessionIdentity {
                protocol_version: PROTOCOL_VERSION,
                plan_id: "plan",
                source_fragment_id: "source-fragment",
                sink_fragment_id: "sink-fragment",
                source_active_play_id: "source-play",
                sink_active_play_id: "sink-play",
                connection_id: "cord",
                source_host_id: "source-host",
                source_boot_id: "source-boot",
                sink_host_id: "sink-host",
                sink_boot_id: "sink-boot",
                value_kind: "value/signal@1",
                limits: SessionLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 9,
                    maximum_buffered_bytes: 9,
                },
            },
            checkpoint: SessionCheckpoint {
                next_sequence: 7,
                transfer: SessionTransferCheckpoint::Accepted(7),
                input_closed: false,
            },
        }
    }

    #[test]
    fn exact_checkpoint_round_trips_without_allocating_a_history() {
        let mut bytes = [0_u8; 512];
        let length = encode_session_checkpoint_into(offer(), &mut bytes, 512).unwrap();
        assert_eq!(
            decode_session_checkpoint(&bytes[..length], 512).unwrap(),
            offer()
        );
    }

    #[test]
    fn malformed_or_trailing_checkpoint_fails_closed() {
        let mut bytes = [0_u8; 512];
        let length = encode_session_checkpoint_into(offer(), &mut bytes, 512).unwrap();
        bytes[0] ^= 1;
        assert_eq!(
            decode_session_checkpoint(&bytes[..length], 512),
            Err(WireError::InvalidMagic)
        );
        bytes[0] ^= 1;
        bytes[length] = 1;
        assert_eq!(
            decode_session_checkpoint(&bytes[..=length], 512),
            Err(WireError::TrailingGarbage)
        );
    }
}
