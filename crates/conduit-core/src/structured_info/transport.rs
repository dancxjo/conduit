use alloc::string::String;
use alloc::vec::Vec;

use super::{
    StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};

const MAGIC: &[u8; 6] = b"CND-SI";
const VERSION: u8 = 1;
const HEADER_BYTES: usize = MAGIC.len() + 1 + 32 + 32 + 4;

pub const MAXIMUM_STRUCTURED_TRANSPORT_BYTES: usize =
    HEADER_BYTES + super::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoTransportRefusal {
    Semantic(StructuredInfoRefusal),
    InvalidBudget,
    SizeExhausted,
    UnsupportedVersion,
    ProfileMismatch,
    ValueIdentityMismatch,
    MalformedRepresentation,
}

/// One bounded versioned remote representation. Local execution need not use it.
pub fn encode_structured_transport(
    value: &StructuredInfoValue,
    maximum_bytes: u32,
) -> Result<Vec<u8>, StructuredInfoTransportRefusal> {
    let maximum_bytes = usize::try_from(maximum_bytes)
        .map_err(|_| StructuredInfoTransportRefusal::InvalidBudget)?;
    if !(HEADER_BYTES..=MAXIMUM_STRUCTURED_TRANSPORT_BYTES).contains(&maximum_bytes) {
        return Err(StructuredInfoTransportRefusal::InvalidBudget);
    }
    let payload = value
        .canonical_bytes()
        .map_err(StructuredInfoTransportRefusal::Semantic)?;
    let length = HEADER_BYTES
        .checked_add(payload.len())
        .ok_or(StructuredInfoTransportRefusal::SizeExhausted)?;
    if length > maximum_bytes {
        return Err(StructuredInfoTransportRefusal::SizeExhausted);
    }
    let mut encoded = Vec::with_capacity(length);
    encoded.extend_from_slice(MAGIC);
    encoded.push(VERSION);
    encoded.extend_from_slice(
        &value
            .value_type()
            .semantic_digest()
            .map_err(StructuredInfoTransportRefusal::Semantic)?,
    );
    encoded.extend_from_slice(
        &value
            .semantic_digest()
            .map_err(StructuredInfoTransportRefusal::Semantic)?,
    );
    encoded.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&payload);
    Ok(encoded)
}

pub fn decode_structured_transport(
    expected_type: &StructuredInfoType,
    encoded: &[u8],
    maximum_bytes: u32,
) -> Result<StructuredInfoValue, StructuredInfoTransportRefusal> {
    let maximum_bytes = usize::try_from(maximum_bytes)
        .map_err(|_| StructuredInfoTransportRefusal::InvalidBudget)?;
    if !(HEADER_BYTES..=MAXIMUM_STRUCTURED_TRANSPORT_BYTES).contains(&maximum_bytes) {
        return Err(StructuredInfoTransportRefusal::InvalidBudget);
    }
    if encoded.len() > maximum_bytes {
        return Err(StructuredInfoTransportRefusal::SizeExhausted);
    }
    if encoded.len() < HEADER_BYTES {
        return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
    }
    if &encoded[..MAGIC.len()] != MAGIC {
        return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
    }
    if encoded[MAGIC.len()] != VERSION {
        return Err(StructuredInfoTransportRefusal::UnsupportedVersion);
    }
    let expected_digest = expected_type
        .semantic_digest()
        .map_err(StructuredInfoTransportRefusal::Semantic)?;
    let digest_offset = MAGIC.len() + 1;
    if encoded[digest_offset..digest_offset + 32] != expected_digest {
        return Err(StructuredInfoTransportRefusal::ProfileMismatch);
    }
    let value_digest_offset = digest_offset + 32;
    let expected_value_digest = &encoded[value_digest_offset..value_digest_offset + 32];
    let length_offset = value_digest_offset + 32;
    let payload_length = u32::from_le_bytes(
        encoded[length_offset..length_offset + 4]
            .try_into()
            .map_err(|_| StructuredInfoTransportRefusal::MalformedRepresentation)?,
    );
    let payload_length = usize::try_from(payload_length)
        .map_err(|_| StructuredInfoTransportRefusal::MalformedRepresentation)?;
    let payload = &encoded[HEADER_BYTES..];
    if payload.len() != payload_length {
        return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
    }
    let type_prefix = expected_type
        .canonical_bytes()
        .map_err(StructuredInfoTransportRefusal::Semantic)?;
    let node = payload
        .strip_prefix(type_prefix.as_slice())
        .ok_or(StructuredInfoTransportRefusal::ProfileMismatch)?;
    let mut cursor = Cursor::new(node);
    let value = decode_node(expected_type, &mut cursor)?;
    if !cursor.remaining().is_empty() {
        return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
    }
    if value
        .semantic_digest()
        .map_err(StructuredInfoTransportRefusal::Semantic)?
        != expected_value_digest
    {
        return Err(StructuredInfoTransportRefusal::ValueIdentityMismatch);
    }
    Ok(value)
}

fn decode_node(
    expected: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
) -> Result<StructuredInfoValue, StructuredInfoTransportRefusal> {
    let tag = cursor.byte()?;
    let decoded = match expected.shape() {
        StructuredInfoTypeShape::Leaf(_) if tag == 0 => {
            StructuredInfoValue::leaf(expected.clone(), cursor.bytes()?.to_vec())
        }
        StructuredInfoTypeShape::Collection { element, length } if tag == 1 => {
            if cursor.length()? != usize::from(length) {
                return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
            }
            let mut values = Vec::with_capacity(usize::from(length));
            for _ in 0..length {
                values.push(decode_node(element, cursor)?);
            }
            StructuredInfoValue::collection(expected.clone(), values)
        }
        StructuredInfoTypeShape::Record { fields, .. } if tag == 2 => {
            if cursor.length()? != fields.len() {
                return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
            }
            let mut values = Vec::with_capacity(fields.len());
            for field in fields {
                let name = cursor.text()?;
                if name != field.name() {
                    return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
                }
                values.push(
                    StructuredFieldValue::new(name, decode_node(field.value_type(), cursor)?)
                        .map_err(StructuredInfoTransportRefusal::Semantic)?,
                );
            }
            StructuredInfoValue::record(expected.clone(), values)
        }
        StructuredInfoTypeShape::Variant { cases, .. } if tag == 3 => {
            let case_tag = cursor.text()?;
            let case = cases
                .iter()
                .find(|case| case.tag() == case_tag)
                .ok_or(StructuredInfoTransportRefusal::MalformedRepresentation)?;
            StructuredInfoValue::variant(
                expected.clone(),
                case_tag,
                decode_node(case.payload_type(), cursor)?,
            )
        }
        _ => return Err(StructuredInfoTransportRefusal::MalformedRepresentation),
    };
    decoded.map_err(StructuredInfoTransportRefusal::Semantic)
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn remaining(&self) -> &'a [u8] {
        self.remaining
    }

    fn byte(&mut self) -> Result<u8, StructuredInfoTransportRefusal> {
        let (&byte, rest) = self
            .remaining
            .split_first()
            .ok_or(StructuredInfoTransportRefusal::MalformedRepresentation)?;
        self.remaining = rest;
        Ok(byte)
    }

    fn length(&mut self) -> Result<usize, StructuredInfoTransportRefusal> {
        let bytes = self.take(4)?;
        usize::try_from(u32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| StructuredInfoTransportRefusal::MalformedRepresentation)?,
        ))
        .map_err(|_| StructuredInfoTransportRefusal::MalformedRepresentation)
    }

    fn bytes(&mut self) -> Result<&'a [u8], StructuredInfoTransportRefusal> {
        let length = self.length()?;
        self.take(length)
    }

    fn text(&mut self) -> Result<String, StructuredInfoTransportRefusal> {
        let bytes = self.bytes()?;
        core::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|_| StructuredInfoTransportRefusal::MalformedRepresentation)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], StructuredInfoTransportRefusal> {
        if length > self.remaining.len() {
            return Err(StructuredInfoTransportRefusal::MalformedRepresentation);
        }
        let (value, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(value)
    }
}
