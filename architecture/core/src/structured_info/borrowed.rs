//! Allocation-free validation of self-describing canonical structured values.

use super::{
    StructuredInfoRefusal, MAXIMUM_STRUCTURED_CANONICAL_BYTES, MAXIMUM_STRUCTURED_COLLECTION_ITEMS,
    MAXIMUM_STRUCTURED_INFO_DEPTH, MAXIMUM_STRUCTURED_INFO_NODES, MAXIMUM_STRUCTURED_LEAF_BYTES,
    MAXIMUM_STRUCTURED_NAME_BYTES, MAXIMUM_STRUCTURED_RECORD_FIELDS,
    MAXIMUM_STRUCTURED_VARIANT_CASES,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ValidatedCanonicalStructuredValue<'a> {
    type_bytes: &'a [u8],
    value_node: &'a [u8],
}

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    pub const fn type_bytes(self) -> &'a [u8] {
        self.type_bytes
    }
    pub const fn value_node(self) -> &'a [u8] {
        self.value_node
    }
    pub fn type_semantic_digest(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(super::TYPE_DIGEST_DOMAIN);
        hash.update((self.type_bytes.len() as u32).to_le_bytes());
        hash.update(self.type_bytes);
        hash.finalize().into()
    }
}

pub fn validate_canonical_structured_value(
    encoded: &[u8],
) -> Result<ValidatedCanonicalStructuredValue<'_>, StructuredInfoRefusal> {
    if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
    }
    let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
    let (type_bytes, value_node) = split_type(encoded, 1, &mut nodes)?;
    let mut value = Cursor::new(value_node);
    let mut value_nodes = MAXIMUM_STRUCTURED_INFO_NODES;
    validate_value(type_bytes, &mut value, 1, &mut value_nodes)?;
    if !value.remaining.is_empty() {
        return Err(malformed());
    }
    Ok(ValidatedCanonicalStructuredValue {
        type_bytes,
        value_node,
    })
}

fn split_type<'a>(
    input: &'a [u8],
    depth: usize,
    nodes: &mut usize,
) -> Result<(&'a [u8], &'a [u8]), StructuredInfoRefusal> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH || *nodes == 0 {
        return Err(malformed());
    }
    *nodes -= 1;
    let mut cursor = Cursor::new(input);
    match cursor.byte()? {
        0 => {
            checked_name(cursor.text()?)?;
        }
        1 => {
            if usize::from(cursor.u16()?) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                return Err(malformed());
            }
            let (_, rest) = split_type(cursor.remaining, depth + 1, nodes)?;
            cursor.remaining = rest;
        }
        tag @ (2 | 3) => {
            checked_name(cursor.text()?)?;
            let count = cursor.length()?;
            let maximum = if tag == 2 {
                MAXIMUM_STRUCTURED_RECORD_FIELDS
            } else {
                MAXIMUM_STRUCTURED_VARIANT_CASES
            };
            if count == 0 || count > maximum {
                return Err(malformed());
            }
            let mut prior = None;
            for _ in 0..count {
                let name = cursor.text()?;
                checked_name(name)?;
                if prior.is_some_and(|prior| prior >= name) {
                    return Err(malformed());
                }
                prior = Some(name);
                let (_, rest) = split_type(cursor.remaining, depth + 1, nodes)?;
                cursor.remaining = rest;
            }
        }
        _ => return Err(malformed()),
    }
    let consumed = input.len() - cursor.remaining.len();
    Ok(input.split_at(consumed))
}

fn validate_value(
    type_bytes: &[u8],
    value: &mut Cursor<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH || *nodes == 0 {
        return Err(malformed());
    }
    *nodes -= 1;
    let mut kind = Cursor::new(type_bytes);
    match (kind.byte()?, value.byte()?) {
        (0, 0) => {
            checked_name(kind.text()?)?;
            if value.bytes()?.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(malformed());
            }
        }
        (1, 1) => {
            let count = usize::from(kind.u16()?);
            if value.length()? != count {
                return Err(malformed());
            }
            let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
            let (element, rest) = split_type(kind.remaining, depth + 1, &mut scratch)?;
            kind.remaining = rest;
            for _ in 0..count {
                validate_value(element, value, depth + 1, nodes)?;
            }
        }
        (tag @ (2 | 3), value_tag) if tag == value_tag => {
            checked_name(kind.text()?)?;
            let count = kind.length()?;
            if tag == 2 {
                if value.length()? != count {
                    return Err(malformed());
                }
                for _ in 0..count {
                    let expected = kind.text()?;
                    if value.text()? != expected {
                        return Err(malformed());
                    }
                    let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
                    let (child, rest) = split_type(kind.remaining, depth + 1, &mut scratch)?;
                    kind.remaining = rest;
                    validate_value(child, value, depth + 1, nodes)?;
                }
            } else {
                let selected = value.text()?;
                let mut selected_type = None;
                for _ in 0..count {
                    let case = kind.text()?;
                    let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
                    let (child, rest) = split_type(kind.remaining, depth + 1, &mut scratch)?;
                    kind.remaining = rest;
                    if case == selected {
                        selected_type = Some(child);
                    }
                }
                validate_value(
                    selected_type.ok_or_else(malformed)?,
                    value,
                    depth + 1,
                    nodes,
                )?;
            }
        }
        _ => return Err(malformed()),
    }
    if !kind.remaining.is_empty() {
        return Err(malformed());
    }
    Ok(())
}

fn checked_name(value: &str) -> Result<(), StructuredInfoRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_STRUCTURED_NAME_BYTES {
        Err(malformed())
    } else {
        Ok(())
    }
}
const fn malformed() -> StructuredInfoRefusal {
    StructuredInfoRefusal::MalformedCanonicalEncoding
}

struct Cursor<'a> {
    remaining: &'a [u8],
}
impl<'a> Cursor<'a> {
    const fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], StructuredInfoRefusal> {
        let (head, tail) = self
            .remaining
            .split_at_checked(length)
            .ok_or_else(malformed)?;
        self.remaining = tail;
        Ok(head)
    }
    fn byte(&mut self) -> Result<u8, StructuredInfoRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, StructuredInfoRefusal> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| malformed())?,
        ))
    }
    fn length(&mut self) -> Result<usize, StructuredInfoRefusal> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| malformed())?) as usize)
    }
    fn bytes(&mut self) -> Result<&'a [u8], StructuredInfoRefusal> {
        let length = self.length()?;
        self.take(length)
    }
    fn text(&mut self) -> Result<&'a str, StructuredInfoRefusal> {
        core::str::from_utf8(self.bytes()?).map_err(|_| malformed())
    }
}
