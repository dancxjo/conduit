use alloc::vec::Vec;
use sha2::{Digest, Sha256};

use super::{
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeNode, StructuredInfoValue,
    StructuredInfoValueNode, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
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
