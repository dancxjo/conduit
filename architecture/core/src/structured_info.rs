//! Canonical finite structured Info schemas and values.
//!
//! This module owns data shape only. It does not add Form syntax, temporal
//! semantics, selection, effects, or a provider-specific object model.

use alloc::string::String;
use alloc::vec::Vec;

use crate::KindId;

mod canonical;
mod inspection;
mod profile;
mod selection;
mod transport;
mod validation;
use canonical::{
    check_encoding_size, decode_type, decode_value, digest, encode_type, encode_value_node,
    type_extent, value_extent,
};
pub use inspection::*;
pub use profile::*;
pub use selection::*;
pub use transport::*;
pub use validation::PreparedStructuredValueValidator;

pub const MAXIMUM_STRUCTURED_INFO_DEPTH: usize = 8;
pub const MAXIMUM_STRUCTURED_INFO_NODES: usize = 1_024;
pub const MAXIMUM_STRUCTURED_COLLECTION_ITEMS: usize = 256;
pub const MAXIMUM_STRUCTURED_RECORD_FIELDS: usize = 64;
pub const MAXIMUM_STRUCTURED_VARIANT_CASES: usize = 64;
pub const MAXIMUM_STRUCTURED_NAME_BYTES: usize = 128;
pub const MAXIMUM_STRUCTURED_LEAF_BYTES: usize = 4_096;
pub const MAXIMUM_STRUCTURED_CANONICAL_BYTES: usize = 65_536;

const TYPE_DIGEST_DOMAIN: &[u8] = b"conduit.structured-info.type.v1";
const VALUE_DIGEST_DOMAIN: &[u8] = b"conduit.structured-info.value.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoRefusal {
    EmptyName,
    NameTooLong,
    DuplicateName,
    EmptyShape,
    UnboundedCollection,
    CollectionTooLarge,
    TooManyFields,
    TooManyCases,
    TooDeep,
    TooManyNodes,
    LeafTooLarge,
    WrongType,
    WrongCollectionLength,
    WrongRecordFields,
    UnknownVariantTag,
    CanonicalEncodingTooLarge,
    MalformedCanonicalEncoding,
}

/// One exact canonical structured Info type.
///
/// Record and variant identities are nominal. Their members are additionally
/// checked structurally, so two protocols cannot become compatible merely by
/// reusing field names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuredInfoType(StructuredInfoTypeNode);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredInfoTypeShape<'a> {
    Leaf(&'a KindId),
    Collection {
        element: &'a StructuredInfoType,
        length: u16,
    },
    Record {
        schema: &'a KindId,
        fields: &'a [StructuredFieldType],
    },
    Variant {
        schema: &'a KindId,
        cases: &'a [StructuredVariantCase],
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StructuredInfoTypeNode {
    Leaf(KindId),
    Collection {
        element: alloc::boxed::Box<StructuredInfoType>,
        length: u16,
    },
    Record {
        schema: KindId,
        fields: Vec<StructuredFieldType>,
    },
    Variant {
        schema: KindId,
        cases: Vec<StructuredVariantCase>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuredFieldType {
    name: String,
    value_type: StructuredInfoType,
}

impl StructuredFieldType {
    pub fn new(
        name: impl Into<String>,
        value_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let name = name.into();
        validate_name(&name)?;
        Ok(Self { name, value_type })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuredVariantCase {
    tag: String,
    payload_type: StructuredInfoType,
}

impl StructuredVariantCase {
    pub fn new(
        tag: impl Into<String>,
        payload_type: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        let tag = tag.into();
        validate_name(&tag)?;
        Ok(Self { tag, payload_type })
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn payload_type(&self) -> &StructuredInfoType {
        &self.payload_type
    }
}

impl StructuredInfoType {
    pub fn shape(&self) -> StructuredInfoTypeShape<'_> {
        match &self.0 {
            StructuredInfoTypeNode::Leaf(kind) => StructuredInfoTypeShape::Leaf(kind),
            StructuredInfoTypeNode::Collection { element, length } => {
                StructuredInfoTypeShape::Collection {
                    element,
                    length: *length,
                }
            }
            StructuredInfoTypeNode::Record { schema, fields } => {
                StructuredInfoTypeShape::Record { schema, fields }
            }
            StructuredInfoTypeNode::Variant { schema, cases } => {
                StructuredInfoTypeShape::Variant { schema, cases }
            }
        }
    }

    pub fn leaf(kind: KindId) -> Result<Self, StructuredInfoRefusal> {
        validate_name(kind.as_str())?;
        Ok(Self(StructuredInfoTypeNode::Leaf(kind)))
    }

    /// An absent length is explicitly unbounded and therefore refused.
    pub fn collection(
        element: StructuredInfoType,
        exact_length: Option<u16>,
    ) -> Result<Self, StructuredInfoRefusal> {
        let length = exact_length.ok_or(StructuredInfoRefusal::UnboundedCollection)?;
        if usize::from(length) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
            return Err(StructuredInfoRefusal::CollectionTooLarge);
        }
        let value = Self(StructuredInfoTypeNode::Collection {
            element: alloc::boxed::Box::new(element),
            length,
        });
        value.validate_limits()?;
        Ok(value)
    }

    pub fn record(
        schema: KindId,
        mut fields: Vec<StructuredFieldType>,
    ) -> Result<Self, StructuredInfoRefusal> {
        validate_name(schema.as_str())?;
        if fields.is_empty() {
            return Err(StructuredInfoRefusal::EmptyShape);
        }
        if fields.len() > MAXIMUM_STRUCTURED_RECORD_FIELDS {
            return Err(StructuredInfoRefusal::TooManyFields);
        }
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        reject_duplicate_names(fields.iter().map(|field| field.name.as_str()))?;
        let value = Self(StructuredInfoTypeNode::Record { schema, fields });
        value.validate_limits()?;
        Ok(value)
    }

    pub fn variant(
        schema: KindId,
        mut cases: Vec<StructuredVariantCase>,
    ) -> Result<Self, StructuredInfoRefusal> {
        validate_name(schema.as_str())?;
        if cases.is_empty() {
            return Err(StructuredInfoRefusal::EmptyShape);
        }
        if cases.len() > MAXIMUM_STRUCTURED_VARIANT_CASES {
            return Err(StructuredInfoRefusal::TooManyCases);
        }
        cases.sort_by(|left, right| left.tag.cmp(&right.tag));
        reject_duplicate_names(cases.iter().map(|case| case.tag.as_str()))?;
        let value = Self(StructuredInfoTypeNode::Variant { schema, cases });
        value.validate_limits()?;
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StructuredInfoRefusal> {
        let mut encoded = Vec::new();
        encode_type(self, &mut encoded);
        check_encoding_size(encoded)
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], StructuredInfoRefusal> {
        digest(TYPE_DIGEST_DOMAIN, &self.canonical_bytes()?)
    }

    fn validate_limits(&self) -> Result<(), StructuredInfoRefusal> {
        let (depth, nodes) = type_extent(self);
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(StructuredInfoRefusal::TooDeep);
        }
        if nodes > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(StructuredInfoRefusal::TooManyNodes);
        }
        self.canonical_bytes().map(|_| ())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredInfoValue {
    value_type: StructuredInfoType,
    node: StructuredInfoValueNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredInfoValueShape<'a> {
    Leaf(&'a [u8]),
    Collection(&'a [StructuredInfoValue]),
    Record(&'a [StructuredFieldValue]),
    Variant {
        tag: &'a str,
        payload: &'a StructuredInfoValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StructuredInfoValueNode {
    Leaf(Vec<u8>),
    Collection(Vec<StructuredInfoValue>),
    Record(Vec<StructuredFieldValue>),
    Variant {
        tag: String,
        payload: alloc::boxed::Box<StructuredInfoValue>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredFieldValue {
    name: String,
    value: StructuredInfoValue,
}

impl StructuredFieldValue {
    pub fn new(
        name: impl Into<String>,
        value: StructuredInfoValue,
    ) -> Result<Self, StructuredInfoRefusal> {
        let name = name.into();
        validate_name(&name)?;
        Ok(Self { name, value })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &StructuredInfoValue {
        &self.value
    }
}

impl StructuredInfoValue {
    pub fn shape(&self) -> StructuredInfoValueShape<'_> {
        match &self.node {
            StructuredInfoValueNode::Leaf(bytes) => StructuredInfoValueShape::Leaf(bytes),
            StructuredInfoValueNode::Collection(values) => {
                StructuredInfoValueShape::Collection(values)
            }
            StructuredInfoValueNode::Record(fields) => StructuredInfoValueShape::Record(fields),
            StructuredInfoValueNode::Variant { tag, payload } => {
                StructuredInfoValueShape::Variant { tag, payload }
            }
        }
    }

    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self, StructuredInfoRefusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let (value_type, remaining) = decode_type(encoded)?;
        let (value, remaining) = decode_value(&value_type, remaining)?;
        if !remaining.is_empty() {
            return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
        }
        Ok(value)
    }

    pub fn leaf(
        value_type: StructuredInfoType,
        canonical_value: Vec<u8>,
    ) -> Result<Self, StructuredInfoRefusal> {
        if !matches!(value_type.0, StructuredInfoTypeNode::Leaf(_)) {
            return Err(StructuredInfoRefusal::WrongType);
        }
        if canonical_value.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
            return Err(StructuredInfoRefusal::LeafTooLarge);
        }
        Self::finish(value_type, StructuredInfoValueNode::Leaf(canonical_value))
    }

    pub fn collection(
        value_type: StructuredInfoType,
        values: Vec<StructuredInfoValue>,
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeNode::Collection { element, length } = &value_type.0 else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        if values.len() != usize::from(*length) {
            return Err(StructuredInfoRefusal::WrongCollectionLength);
        }
        if values.iter().any(|value| value.value_type != **element) {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Self::finish(value_type, StructuredInfoValueNode::Collection(values))
    }

    pub fn record(
        value_type: StructuredInfoType,
        mut fields: Vec<StructuredFieldValue>,
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeNode::Record {
            fields: field_types,
            ..
        } = &value_type.0
        else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        reject_duplicate_names(fields.iter().map(|field| field.name.as_str()))?;
        if fields.len() != field_types.len()
            || fields.iter().zip(field_types).any(|(value, expected)| {
                value.name != expected.name || value.value.value_type != expected.value_type
            })
        {
            return Err(StructuredInfoRefusal::WrongRecordFields);
        }
        Self::finish(value_type, StructuredInfoValueNode::Record(fields))
    }

    pub fn variant(
        value_type: StructuredInfoType,
        tag: impl Into<String>,
        payload: StructuredInfoValue,
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeNode::Variant { cases, .. } = &value_type.0 else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let tag = tag.into();
        validate_name(&tag)?;
        let case = cases
            .iter()
            .find(|case| case.tag == tag)
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?;
        if payload.value_type != case.payload_type {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Self::finish(
            value_type,
            StructuredInfoValueNode::Variant {
                tag,
                payload: alloc::boxed::Box::new(payload),
            },
        )
    }

    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, StructuredInfoRefusal> {
        let mut encoded = self.value_type.canonical_bytes()?;
        encode_value_node(&self.node, &mut encoded);
        check_encoding_size(encoded)
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], StructuredInfoRefusal> {
        digest(VALUE_DIGEST_DOMAIN, &self.canonical_bytes()?)
    }

    fn finish(
        value_type: StructuredInfoType,
        node: StructuredInfoValueNode,
    ) -> Result<Self, StructuredInfoRefusal> {
        let value = Self { value_type, node };
        let (_, nodes) = value_extent(&value);
        if nodes > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(StructuredInfoRefusal::TooManyNodes);
        }
        value.canonical_bytes()?;
        Ok(value)
    }
}

/// A checked startup value. It shares data shape with runtime Info without
/// acquiring a runtime temporal mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupStructuredValue(StructuredInfoValue);

impl StartupStructuredValue {
    pub fn new(value: StructuredInfoValue) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &StructuredInfoValue {
        &self.0
    }
}

/// Runtime Info carried by an already temporal typed Port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStructuredInfo(StructuredInfoValue);

impl RuntimeStructuredInfo {
    pub fn new(value: StructuredInfoValue) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &StructuredInfoValue {
        &self.0
    }
}

fn validate_name(name: &str) -> Result<(), StructuredInfoRefusal> {
    if name.is_empty() {
        return Err(StructuredInfoRefusal::EmptyName);
    }
    if name.len() > MAXIMUM_STRUCTURED_NAME_BYTES {
        return Err(StructuredInfoRefusal::NameTooLong);
    }
    Ok(())
}

fn reject_duplicate_names<'a>(
    names: impl Iterator<Item = &'a str>,
) -> Result<(), StructuredInfoRefusal> {
    let mut previous = None;
    for name in names {
        if previous == Some(name) {
            return Err(StructuredInfoRefusal::DuplicateName);
        }
        previous = Some(name);
    }
    Ok(())
}
