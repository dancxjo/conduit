//! Transport-neutral finite framing for one exactly typed semantic record.

use alloc::vec::Vec;
use conduit_core::{
    kind_id, semantic_digest, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
    MAXIMUM_STRUCTURED_LEAF_BYTES,
};

pub const TYPED_RECORD_INFO_ID: &str = "record/typed@1";
pub const FRAMED_TYPED_RECORD_INFO_ID: &str = "record/framed-typed@1";
pub const TYPED_RECORD_FRAME_VERSION: u8 = 1;
pub const MAXIMUM_TYPED_RECORD_KIND_BYTES: usize = 128;
pub const MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES: usize = MAXIMUM_STRUCTURED_LEAF_BYTES
    - TYPED_RECORD_FRAME_HEADER_BYTES
    - MAXIMUM_TYPED_RECORD_KIND_BYTES;
pub const TYPED_RECORD_FRAME_HEADER_BYTES: usize = 43;
pub const MAXIMUM_TYPED_RECORD_FRAME_BYTES: usize = TYPED_RECORD_FRAME_HEADER_BYTES
    + MAXIMUM_TYPED_RECORD_KIND_BYTES
    + MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES;

const MAGIC: [u8; 4] = *b"CTR1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TypedRecordRef<'a> {
    value_kind: &'a str,
    payload: &'a [u8],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypedRecordFrameRefusal {
    MissingValueKind,
    ValueKindTooLarge,
    InvalidValueKindEncoding,
    PayloadTooLarge,
    FrameTooLarge,
    MalformedPayload,
    PayloadTypeMismatch,
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    LengthOverflow,
    TrailingBytes,
    IntegrityMismatch,
    WrongTypedRecordValueType,
    WrongFramedTypedRecordValueType,
}

pub fn typed_record_value(
    value: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TypedRecordFrameRefusal> {
    let payload = value
        .canonical_bytes()
        .map_err(|_| TypedRecordFrameRefusal::MalformedPayload)?;
    let profile = value
        .value_type()
        .profile()
        .map_err(|_| TypedRecordFrameRefusal::MalformedPayload)?;
    let kind = profile.value_kind().as_str();
    TypedRecordRef::new(kind, &payload)?;
    let mut encoded = Vec::with_capacity(2 + kind.len() + payload.len());
    encoded.extend_from_slice(&(kind.len() as u16).to_le_bytes());
    encoded.extend_from_slice(kind.as_bytes());
    encoded.extend_from_slice(&payload);
    StructuredInfoValue::leaf(typed_record_value_type(), encoded)
        .map_err(|_| TypedRecordFrameRefusal::PayloadTooLarge)
}

pub fn value_from_typed_record(
    value: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TypedRecordFrameRefusal> {
    if value.value_type() != &typed_record_value_type() {
        return Err(TypedRecordFrameRefusal::WrongTypedRecordValueType);
    }
    let StructuredInfoValueShape::Leaf(encoded) = value.shape() else {
        return Err(TypedRecordFrameRefusal::WrongTypedRecordValueType);
    };
    let (kind, payload) = decode_typed_value_bytes(encoded)?;
    TypedRecordRef::new(kind, payload)?;
    StructuredInfoValue::from_canonical_bytes(payload)
        .map_err(|_| TypedRecordFrameRefusal::MalformedPayload)
}

pub fn frame_typed_record_value_into(
    value: &StructuredInfoValue,
    output: &mut [u8],
) -> Result<usize, TypedRecordFrameRefusal> {
    if value.value_type() != &typed_record_value_type() {
        return Err(TypedRecordFrameRefusal::WrongTypedRecordValueType);
    }
    let StructuredInfoValueShape::Leaf(encoded) = value.shape() else {
        return Err(TypedRecordFrameRefusal::WrongTypedRecordValueType);
    };
    let (kind, payload) = decode_typed_value_bytes(encoded)?;
    encode_typed_record_into(TypedRecordRef::new(kind, payload)?, output)
}

pub fn framed_typed_record_value(
    frame: &[u8],
) -> Result<StructuredInfoValue, TypedRecordFrameRefusal> {
    decode_typed_record(frame)?;
    StructuredInfoValue::leaf(framed_typed_record_value_type(), frame.to_vec())
        .map_err(|_| TypedRecordFrameRefusal::FrameTooLarge)
}

pub fn deframe_typed_record_value(
    framed: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TypedRecordFrameRefusal> {
    if framed.value_type() != &framed_typed_record_value_type() {
        return Err(TypedRecordFrameRefusal::WrongFramedTypedRecordValueType);
    }
    let StructuredInfoValueShape::Leaf(frame) = framed.shape() else {
        return Err(TypedRecordFrameRefusal::WrongFramedTypedRecordValueType);
    };
    let record = decode_typed_record(frame)?;
    let value = StructuredInfoValue::from_canonical_bytes(record.payload())
        .map_err(|_| TypedRecordFrameRefusal::MalformedPayload)?;
    typed_record_value(&value)
}

fn typed_record_value_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(TYPED_RECORD_INFO_ID)).expect("typed record value type")
}

fn framed_typed_record_value_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(FRAMED_TYPED_RECORD_INFO_ID))
        .expect("framed typed record value type")
}

fn decode_typed_value_bytes(encoded: &[u8]) -> Result<(&str, &[u8]), TypedRecordFrameRefusal> {
    if encoded.len() < 2 {
        return Err(TypedRecordFrameRefusal::MalformedPayload);
    }
    let kind_length = usize::from(u16::from_le_bytes([encoded[0], encoded[1]]));
    let kind_end = 2_usize
        .checked_add(kind_length)
        .ok_or(TypedRecordFrameRefusal::LengthOverflow)?;
    let kind = core::str::from_utf8(
        encoded
            .get(2..kind_end)
            .ok_or(TypedRecordFrameRefusal::MalformedPayload)?,
    )
    .map_err(|_| TypedRecordFrameRefusal::InvalidValueKindEncoding)?;
    let payload = encoded
        .get(kind_end..)
        .ok_or(TypedRecordFrameRefusal::MalformedPayload)?;
    Ok((kind, payload))
}

impl<'a> TypedRecordRef<'a> {
    pub fn new(value_kind: &'a str, payload: &'a [u8]) -> Result<Self, TypedRecordFrameRefusal> {
        let record = Self {
            value_kind,
            payload,
        };
        validate_record(record)?;
        Ok(record)
    }

    pub const fn value_kind(self) -> &'a str {
        self.value_kind
    }

    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }
}

pub fn encode_typed_record_into(
    record: TypedRecordRef<'_>,
    output: &mut [u8],
) -> Result<usize, TypedRecordFrameRefusal> {
    validate_record(record)?;
    let encoded_len = TYPED_RECORD_FRAME_HEADER_BYTES
        .checked_add(record.value_kind.len())
        .and_then(|length| length.checked_add(record.payload.len()))
        .ok_or(TypedRecordFrameRefusal::LengthOverflow)?;
    if output.len() < encoded_len {
        return Err(TypedRecordFrameRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = TYPED_RECORD_FRAME_VERSION;
    output[5..7].copy_from_slice(&(record.value_kind.len() as u16).to_le_bytes());
    output[7..11].copy_from_slice(&(record.payload.len() as u32).to_le_bytes());
    let content_start = 11;
    let payload_start = content_start + record.value_kind.len();
    let integrity_start = payload_start + record.payload.len();
    output[content_start..payload_start].copy_from_slice(record.value_kind.as_bytes());
    output[payload_start..integrity_start].copy_from_slice(record.payload);
    let integrity = semantic_digest(TYPED_RECORD_INFO_ID, &output[4..integrity_start]);
    output[integrity_start..encoded_len].copy_from_slice(&integrity);
    Ok(encoded_len)
}

pub fn decode_typed_record(frame: &[u8]) -> Result<TypedRecordRef<'_>, TypedRecordFrameRefusal> {
    if frame.len() < TYPED_RECORD_FRAME_HEADER_BYTES {
        return Err(TypedRecordFrameRefusal::Truncated);
    }
    if frame[..4] != MAGIC {
        return Err(TypedRecordFrameRefusal::InvalidMagic);
    }
    if frame[4] != TYPED_RECORD_FRAME_VERSION {
        return Err(TypedRecordFrameRefusal::UnsupportedVersion);
    }
    let value_kind_len = usize::from(u16::from_le_bytes([frame[5], frame[6]]));
    let payload_len = usize::try_from(u32::from_le_bytes([
        frame[7], frame[8], frame[9], frame[10],
    ]))
    .map_err(|_| TypedRecordFrameRefusal::LengthOverflow)?;
    if value_kind_len == 0 {
        return Err(TypedRecordFrameRefusal::MissingValueKind);
    }
    if value_kind_len > MAXIMUM_TYPED_RECORD_KIND_BYTES {
        return Err(TypedRecordFrameRefusal::ValueKindTooLarge);
    }
    if payload_len > MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES {
        return Err(TypedRecordFrameRefusal::PayloadTooLarge);
    }
    let encoded_len = TYPED_RECORD_FRAME_HEADER_BYTES
        .checked_add(value_kind_len)
        .and_then(|length| length.checked_add(payload_len))
        .ok_or(TypedRecordFrameRefusal::LengthOverflow)?;
    if frame.len() < encoded_len {
        return Err(TypedRecordFrameRefusal::Truncated);
    }
    if frame.len() > encoded_len {
        return Err(TypedRecordFrameRefusal::TrailingBytes);
    }
    let content_start = 11;
    let payload_start = content_start + value_kind_len;
    let integrity_start = payload_start + payload_len;
    let value_kind = core::str::from_utf8(&frame[content_start..payload_start])
        .map_err(|_| TypedRecordFrameRefusal::InvalidValueKindEncoding)?;
    let expected = semantic_digest(TYPED_RECORD_INFO_ID, &frame[4..integrity_start]);
    if frame[integrity_start..encoded_len] != expected {
        return Err(TypedRecordFrameRefusal::IntegrityMismatch);
    }
    TypedRecordRef::new(value_kind, &frame[payload_start..integrity_start])
}

fn validate_record(record: TypedRecordRef<'_>) -> Result<(), TypedRecordFrameRefusal> {
    if record.value_kind.is_empty() {
        return Err(TypedRecordFrameRefusal::MissingValueKind);
    }
    if record.value_kind.len() > MAXIMUM_TYPED_RECORD_KIND_BYTES {
        return Err(TypedRecordFrameRefusal::ValueKindTooLarge);
    }
    if record.payload.len() > MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES {
        return Err(TypedRecordFrameRefusal::PayloadTooLarge);
    }
    let value = conduit_core::validate_canonical_structured_value(record.payload)
        .map_err(|_| TypedRecordFrameRefusal::MalformedPayload)?;
    if !profile_matches(value.type_semantic_digest(), record.value_kind) {
        return Err(TypedRecordFrameRefusal::PayloadTypeMismatch);
    }
    Ok(())
}

fn profile_matches(digest: [u8; 32], candidate: &str) -> bool {
    let Some(hex) = candidate
        .strip_prefix("structured-info/profile-")
        .and_then(|value| value.strip_suffix("@1"))
    else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .zip(digest)
        .all(|(pair, byte)| {
            *pair
                == [
                    DIGITS[usize::from(byte >> 4)],
                    DIGITS[usize::from(byte & 0x0f)],
                ]
        })
}
