#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{ConnectionEnvelope, ConnectionId, KindId, PlanId, PROTOCOL_VERSION};

const MAGIC: [u8; 4] = *b"CNDW";
const WIRE_FORMAT_VERSION: u8 = 1;
pub const MAX_ID_BYTES: usize = 4_096;
const FIXED_FRAME_BYTES: usize = 4 + 1 + 2 + 2 + 2 + 8 + 2 + 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WireError {
    InvalidMagic,
    UnsupportedWireFormat,
    WrongProtocolVersion,
    TruncatedFrame,
    OversizedFrame,
    OversizedPayload,
    IdentifierTooLong,
    InvalidIdentifierEncoding,
    TrailingGarbage,
}

pub fn encode_envelope(
    envelope: &ConnectionEnvelope,
    maximum_payload_bytes: u32,
) -> Result<Vec<u8>, WireError> {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    if envelope.payload.len() > maximum_payload_bytes as usize {
        return Err(WireError::OversizedPayload);
    }
    for identity in [
        envelope.plan_id.as_str(),
        envelope.connection_id.as_str(),
        envelope.value_kind.as_str(),
    ] {
        if identity.len() > MAX_ID_BYTES || identity.len() > u16::MAX as usize {
            return Err(WireError::IdentifierTooLong);
        }
    }

    let mut frame = Vec::with_capacity(
        FIXED_FRAME_BYTES
            + envelope.plan_id.as_str().len()
            + envelope.connection_id.as_str().len()
            + envelope.value_kind.as_str().len()
            + envelope.payload.len(),
    );
    frame.extend_from_slice(&MAGIC);
    frame.push(WIRE_FORMAT_VERSION);
    frame.extend_from_slice(&envelope.protocol_version.to_le_bytes());
    push_string(&mut frame, envelope.plan_id.as_str());
    push_string(&mut frame, envelope.connection_id.as_str());
    frame.extend_from_slice(&envelope.sequence.to_le_bytes());
    push_string(&mut frame, envelope.value_kind.as_str());
    frame.extend_from_slice(&(envelope.payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&envelope.payload);
    Ok(frame)
}

pub fn decode_envelope(
    frame: &[u8],
    maximum_payload_bytes: u32,
) -> Result<ConnectionEnvelope, WireError> {
    let maximum_frame_bytes = FIXED_FRAME_BYTES + MAX_ID_BYTES * 3 + maximum_payload_bytes as usize;
    if frame.len() > maximum_frame_bytes {
        return Err(WireError::OversizedFrame);
    }

    let mut cursor = Cursor::new(frame);
    if cursor.take(4)? != MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if cursor.read_u8()? != WIRE_FORMAT_VERSION {
        return Err(WireError::UnsupportedWireFormat);
    }
    let protocol_version = cursor.read_u16()?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let plan_id = PlanId::from(cursor.read_string()?);
    let connection_id = ConnectionId::from(cursor.read_string()?);
    let sequence = cursor.read_u64()?;
    let value_kind = KindId::from(cursor.read_string()?);
    let payload_len = cursor.read_u32()? as usize;
    if payload_len > maximum_payload_bytes as usize {
        return Err(WireError::OversizedPayload);
    }
    let payload = cursor.take(payload_len)?.to_vec();
    if !cursor.is_empty() {
        return Err(WireError::TrailingGarbage);
    }
    Ok(ConnectionEnvelope {
        protocol_version,
        plan_id,
        connection_id,
        sequence,
        value_kind,
        payload,
    })
}

fn push_string(frame: &mut Vec<u8>, value: &str) {
    frame.extend_from_slice(&(value.len() as u16).to_le_bytes());
    frame.extend_from_slice(value.as_bytes());
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

    fn read_u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, WireError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, WireError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_string(&mut self) -> Result<String, WireError> {
        let length = self.read_u16()? as usize;
        if length > MAX_ID_BYTES {
            return Err(WireError::IdentifierTooLong);
        }
        let bytes = self.take(length)?;
        let value =
            core::str::from_utf8(bytes).map_err(|_| WireError::InvalidIdentifierEncoding)?;
        Ok(String::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_envelope, encode_envelope, WireError};
    use alloc::vec;
    use conduit_core::{ConnectionEnvelope, ConnectionId, KindId, PlanId, PROTOCOL_VERSION};

    const MAX_PAYLOAD: u32 = 16;

    fn envelope() -> ConnectionEnvelope {
        ConnectionEnvelope {
            protocol_version: PROTOCOL_VERSION,
            plan_id: PlanId::from("plan-a"),
            connection_id: ConnectionId::from("connection-b"),
            sequence: 0x0102_0304_0506_0708,
            value_kind: KindId::from("test/value"),
            payload: vec![0, 1, 2, 0xff],
        }
    }

    #[test]
    fn deterministic_vector_round_trips_every_envelope_field() {
        let encoded = encode_envelope(&envelope(), MAX_PAYLOAD).expect("envelope encodes");
        assert_eq!(
            encoded,
            b"CNDW\x01\x01\x00\x06\x00plan-a\x0c\x00connection-b\x08\x07\x06\x05\x04\x03\x02\x01\x0a\x00test/value\x04\x00\x00\x00\x00\x01\x02\xff"
        );
        assert_eq!(
            decode_envelope(&encoded, MAX_PAYLOAD).expect("vector decodes"),
            envelope()
        );
    }

    #[test]
    fn rejects_wrong_protocol_truncation_oversize_and_trailing_garbage() {
        let mut invalid_magic = encode_envelope(&envelope(), MAX_PAYLOAD).expect("fixture encodes");
        invalid_magic[0] = b'X';
        assert_eq!(
            decode_envelope(&invalid_magic, MAX_PAYLOAD),
            Err(WireError::InvalidMagic)
        );

        let mut wrong_protocol =
            encode_envelope(&envelope(), MAX_PAYLOAD).expect("fixture encodes");
        wrong_protocol[5..7].copy_from_slice(&(PROTOCOL_VERSION + 1).to_le_bytes());
        assert_eq!(
            decode_envelope(&wrong_protocol, MAX_PAYLOAD),
            Err(WireError::WrongProtocolVersion)
        );

        let encoded = encode_envelope(&envelope(), MAX_PAYLOAD).expect("fixture encodes");
        assert_eq!(
            decode_envelope(&encoded[..encoded.len() - 1], MAX_PAYLOAD),
            Err(WireError::TruncatedFrame)
        );

        let mut oversized = envelope();
        oversized.payload = vec![0; MAX_PAYLOAD as usize + 1];
        assert_eq!(
            encode_envelope(&oversized, MAX_PAYLOAD),
            Err(WireError::OversizedPayload)
        );
        let oversized_encoded =
            encode_envelope(&oversized, MAX_PAYLOAD + 1).expect("larger bound encodes fixture");
        assert_eq!(
            decode_envelope(&oversized_encoded, MAX_PAYLOAD),
            Err(WireError::OversizedPayload)
        );

        let oversized_frame = vec![0; 4 + 1 + 2 + 2 + 2 + 8 + 2 + 4 + 4_096 * 3 + 17];
        assert_eq!(
            decode_envelope(&oversized_frame, MAX_PAYLOAD),
            Err(WireError::OversizedFrame)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            decode_envelope(&trailing, MAX_PAYLOAD),
            Err(WireError::TrailingGarbage)
        );
    }
}
