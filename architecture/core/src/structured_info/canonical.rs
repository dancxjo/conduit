use alloc::string::String;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

use super::{
    StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeNode, StructuredInfoValue, StructuredInfoValueNode, StructuredVariantCase,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub(super) fn type_extent(value: &StructuredInfoType) -> (usize, usize) {
    match &value.0 {
        StructuredInfoTypeNode::Leaf(_) => (1, 1),
        StructuredInfoTypeNode::Collection { element, .. } => {
            let (depth, nodes) = type_extent(element);
            (depth + 1, nodes + 1)
        }
        StructuredInfoTypeNode::Record { fields, .. } => {
            aggregate_type_extent(fields.iter().map(|field| &field.value_type))
        }
        StructuredInfoTypeNode::Variant { cases, .. } => {
            aggregate_type_extent(cases.iter().map(|case| &case.payload_type))
        }
    }
}

fn aggregate_type_extent<'a>(
    children: impl Iterator<Item = &'a StructuredInfoType>,
) -> (usize, usize) {
    children.fold((1, 1), |(depth, nodes), child| {
        let (child_depth, child_nodes) = type_extent(child);
        (depth.max(child_depth + 1), nodes + child_nodes)
    })
}

pub(super) fn value_extent(value: &StructuredInfoValue) -> (usize, usize) {
    match &value.node {
        StructuredInfoValueNode::Leaf(_) => (1, 1),
        StructuredInfoValueNode::Collection(values) => aggregate_value_extent(values.iter()),
        StructuredInfoValueNode::Record(fields) => {
            aggregate_value_extent(fields.iter().map(|field| &field.value))
        }
        StructuredInfoValueNode::Variant { payload, .. } => {
            let (depth, nodes) = value_extent(payload);
            (depth + 1, nodes + 1)
        }
    }
}

fn aggregate_value_extent<'a>(
    children: impl Iterator<Item = &'a StructuredInfoValue>,
) -> (usize, usize) {
    children.fold((1, 1), |(depth, nodes), child| {
        let (child_depth, child_nodes) = value_extent(child);
        (depth.max(child_depth + 1), nodes + child_nodes)
    })
}

pub(super) fn encode_type(value: &StructuredInfoType, out: &mut Vec<u8>) {
    match &value.0 {
        StructuredInfoTypeNode::Leaf(kind) => {
            out.push(0);
            encode_text(kind.as_str(), out);
        }
        StructuredInfoTypeNode::Collection { element, length } => {
            out.push(1);
            out.extend_from_slice(&length.to_le_bytes());
            encode_type(element, out);
        }
        StructuredInfoTypeNode::Record { schema, fields } => {
            out.push(2);
            encode_text(schema.as_str(), out);
            encode_length(fields.len(), out);
            for field in fields {
                encode_text(&field.name, out);
                encode_type(&field.value_type, out);
            }
        }
        StructuredInfoTypeNode::Variant { schema, cases } => {
            out.push(3);
            encode_text(schema.as_str(), out);
            encode_length(cases.len(), out);
            for case in cases {
                encode_text(&case.tag, out);
                encode_type(&case.payload_type, out);
            }
        }
    }
}

pub(super) fn encode_value_node(value: &StructuredInfoValueNode, out: &mut Vec<u8>) {
    match value {
        StructuredInfoValueNode::Leaf(bytes) => {
            out.push(0);
            encode_bytes(bytes, out);
        }
        StructuredInfoValueNode::Collection(values) => {
            out.push(1);
            encode_length(values.len(), out);
            for value in values {
                encode_value_node(&value.node, out);
            }
        }
        StructuredInfoValueNode::Record(fields) => {
            out.push(2);
            encode_length(fields.len(), out);
            for field in fields {
                encode_text(&field.name, out);
                encode_value_node(&field.value.node, out);
            }
        }
        StructuredInfoValueNode::Variant { tag, payload } => {
            out.push(3);
            encode_text(tag, out);
            encode_value_node(&payload.node, out);
        }
    }
}

pub(super) fn decode_type(
    encoded: &[u8],
) -> Result<(StructuredInfoType, &[u8]), StructuredInfoRefusal> {
    let mut cursor = Cursor::new(encoded);
    let mut remaining_nodes = super::MAXIMUM_STRUCTURED_INFO_NODES;
    let value = decode_type_node(&mut cursor, 1, &mut remaining_nodes)?;
    Ok((value, cursor.remaining))
}

fn decode_type_node(
    cursor: &mut Cursor<'_>,
    depth: usize,
    remaining_nodes: &mut usize,
) -> Result<StructuredInfoType, StructuredInfoRefusal> {
    if depth > super::MAXIMUM_STRUCTURED_INFO_DEPTH || *remaining_nodes == 0 {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    *remaining_nodes -= 1;
    match cursor.byte()? {
        0 => StructuredInfoType::leaf(crate::KindId::from(cursor.text()?)),
        1 => {
            let length = cursor.u16()?;
            StructuredInfoType::collection(
                decode_type_node(cursor, depth + 1, remaining_nodes)?,
                Some(length),
            )
        }
        2 => {
            let schema = crate::KindId::from(cursor.text()?);
            let length = cursor.length()?;
            if length > super::MAXIMUM_STRUCTURED_RECORD_FIELDS {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            let mut fields = Vec::with_capacity(length);
            for _ in 0..length {
                fields.push(StructuredFieldType::new(
                    cursor.text()?,
                    decode_type_node(cursor, depth + 1, remaining_nodes)?,
                )?);
            }
            StructuredInfoType::record(schema, fields)
        }
        3 => {
            let schema = crate::KindId::from(cursor.text()?);
            let length = cursor.length()?;
            if length > super::MAXIMUM_STRUCTURED_VARIANT_CASES {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            let mut cases = Vec::with_capacity(length);
            for _ in 0..length {
                cases.push(StructuredVariantCase::new(
                    cursor.text()?,
                    decode_type_node(cursor, depth + 1, remaining_nodes)?,
                )?);
            }
            StructuredInfoType::variant(schema, cases)
        }
        _ => Err(StructuredInfoRefusal::MalformedCanonicalEncoding),
    }
}

pub(super) fn decode_value<'a>(
    expected: &StructuredInfoType,
    encoded: &'a [u8],
) -> Result<(StructuredInfoValue, &'a [u8]), StructuredInfoRefusal> {
    let mut cursor = Cursor::new(encoded);
    let value = decode_value_node(expected, &mut cursor)?;
    Ok((value, cursor.remaining))
}

pub(super) fn validate_value<'a>(
    expected: &StructuredInfoType,
    encoded: &'a [u8],
) -> Result<&'a [u8], StructuredInfoRefusal> {
    let mut cursor = Cursor::new(encoded);
    validate_value_node(expected, &mut cursor)?;
    Ok(cursor.remaining)
}

fn validate_value_node(
    expected: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
) -> Result<(), StructuredInfoRefusal> {
    match (expected.shape(), cursor.byte()?) {
        (super::StructuredInfoTypeShape::Leaf(_), 0) => {
            cursor.bytes()?;
        }
        (super::StructuredInfoTypeShape::Collection { element, length }, 1) => {
            if cursor.length()? != usize::from(length) {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            for _ in 0..length {
                validate_value_node(element, cursor)?;
            }
        }
        (super::StructuredInfoTypeShape::Record { fields, .. }, 2) => {
            if cursor.length()? != fields.len() {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            for field in fields {
                if cursor.text_ref()? != field.name() {
                    return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
                }
                validate_value_node(field.value_type(), cursor)?;
            }
        }
        (super::StructuredInfoTypeShape::Variant { cases, .. }, 3) => {
            let tag = cursor.text_ref()?;
            let case = cases
                .iter()
                .find(|case| case.tag() == tag)
                .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)?;
            validate_value_node(case.payload_type(), cursor)?;
        }
        _ => return Err(StructuredInfoRefusal::MalformedCanonicalEncoding),
    }
    Ok(())
}

fn decode_value_node(
    expected: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    match (expected.shape(), cursor.byte()?) {
        (super::StructuredInfoTypeShape::Leaf(_), 0) => {
            StructuredInfoValue::leaf(expected.clone(), cursor.bytes()?.to_vec())
        }
        (super::StructuredInfoTypeShape::Collection { element, length }, 1) => {
            if cursor.length()? != usize::from(length) {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            let mut values = Vec::with_capacity(usize::from(length));
            for _ in 0..length {
                values.push(decode_value_node(element, cursor)?);
            }
            StructuredInfoValue::collection(expected.clone(), values)
        }
        (super::StructuredInfoTypeShape::Record { fields, .. }, 2) => {
            if cursor.length()? != fields.len() {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            let mut values = Vec::with_capacity(fields.len());
            for field in fields {
                let name = cursor.text()?;
                if name != field.name() {
                    return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
                }
                values.push(StructuredFieldValue::new(
                    name,
                    decode_value_node(field.value_type(), cursor)?,
                )?);
            }
            StructuredInfoValue::record(expected.clone(), values)
        }
        (super::StructuredInfoTypeShape::Variant { cases, .. }, 3) => {
            let tag = cursor.text()?;
            let case = cases
                .iter()
                .find(|case| case.tag() == tag)
                .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)?;
            StructuredInfoValue::variant(
                expected.clone(),
                tag,
                decode_value_node(case.payload_type(), cursor)?,
            )
        }
        _ => Err(StructuredInfoRefusal::MalformedCanonicalEncoding),
    }
}

pub(super) struct Cursor<'a> {
    pub(super) remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    pub(super) fn byte(&mut self) -> Result<u8, StructuredInfoRefusal> {
        let (&byte, rest) = self
            .remaining
            .split_first()
            .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)?;
        self.remaining = rest;
        Ok(byte)
    }

    pub(super) fn u16(&mut self) -> Result<u16, StructuredInfoRefusal> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().map_err(|_| {
            StructuredInfoRefusal::MalformedCanonicalEncoding
        })?))
    }

    pub(super) fn length(&mut self) -> Result<usize, StructuredInfoRefusal> {
        let bytes = self.take(4)?;
        usize::try_from(u32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?,
        ))
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
    }

    pub(super) fn bytes(&mut self) -> Result<&'a [u8], StructuredInfoRefusal> {
        let length = self.length()?;
        self.take(length)
    }

    pub(super) fn text(&mut self) -> Result<String, StructuredInfoRefusal> {
        self.text_ref().map(String::from)
    }

    pub(super) fn text_ref(&mut self) -> Result<&'a str, StructuredInfoRefusal> {
        core::str::from_utf8(self.bytes()?)
            .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], StructuredInfoRefusal> {
        if length > self.remaining.len() {
            return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
        }
        let (value, rest) = self.remaining.split_at(length);
        self.remaining = rest;
        Ok(value)
    }
}

fn encode_text(value: &str, out: &mut Vec<u8>) {
    encode_bytes(value.as_bytes(), out);
}

fn encode_bytes(value: &[u8], out: &mut Vec<u8>) {
    encode_length(value.len(), out);
    out.extend_from_slice(value);
}

fn encode_length(value: usize, out: &mut Vec<u8>) {
    out.extend_from_slice(&(value as u32).to_le_bytes());
}

pub(super) fn check_encoding_size(encoded: Vec<u8>) -> Result<Vec<u8>, StructuredInfoRefusal> {
    if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
    } else {
        Ok(encoded)
    }
}

pub(super) fn digest(domain: &[u8], encoded: &[u8]) -> Result<[u8; 32], StructuredInfoRefusal> {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((encoded.len() as u32).to_le_bytes());
    hash.update(encoded);
    Ok(hash.finalize().into())
}
