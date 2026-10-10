//! Canonical finite structured Info schemas and values.
//!
//! This module owns data shape only. It does not add Plot syntax, temporal
//! semantics, selection, effects, or a provider-specific object model.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::{validate_primitive_info, KindId, PrimitiveInfoRefusal};

mod borrowed;
mod canonical;
pub use borrowed::*;
mod inspection;
mod prepared_composition;
mod profile;
mod selection;
mod sequence;
mod transport;
mod tuple;
mod typed_pair;
mod validation;
use canonical::{
    check_encoding_size, decode_type, decode_value, digest, encode_type, encode_value_node,
    type_extent, validate_value, value_extent,
};
pub use inspection::*;
pub use prepared_composition::*;
pub use profile::*;
pub use selection::*;
pub use sequence::*;
pub use transport::*;
pub use tuple::*;
pub use typed_pair::*;
pub use validation::PreparedStructuredValueValidator;

pub const MAXIMUM_STRUCTURED_INFO_DEPTH: usize = 16;
/// Aggregate-node ceiling for one structured type or value.
///
/// A single collection remains capped at 1,024 items. Larger finite semantic
/// families compose bounded pages, so their total node budget must accommodate
/// the established 4,096-item AI limits plus nominal and record framing. The
/// canonical 256 KiB envelope remains the tighter byte bound.
pub const MAXIMUM_STRUCTURED_INFO_NODES: usize = 16_384;
pub const MAXIMUM_STRUCTURED_COLLECTION_ITEMS: usize = 1_024;
pub const MAXIMUM_STRUCTURED_RECORD_FIELDS: usize = 64;
pub const MAXIMUM_STRUCTURED_VARIANT_CASES: usize = 64;
pub const MAXIMUM_STRUCTURED_NAME_BYTES: usize = 128;
/// Largest authored primitive payload. This matches the portable `Bytes`
/// ceiling; the canonical envelope remains separately and finitely bounded.
pub const MAXIMUM_STRUCTURED_LEAF_BYTES: usize = 65_536;
/// Room for two maximum-sized leaves plus finite aggregate type/value framing.
/// This lets one bounded structured value carry a paged 128 KiB semantic payload.
pub const MAXIMUM_STRUCTURED_CANONICAL_BYTES: usize = 262_144;

const TYPE_DIGEST_DOMAIN: &[u8] = b"conduit.structured-info.type.v1";
const VALUE_DIGEST_DOMAIN: &[u8] = b"conduit.structured-info.value.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredInfoRefusal {
    EmptyName,
    NameTooLong,
    DuplicateName,
    EmptyShape,
    InvalidCollectionBounds,
    UnboundedCollection,
    CollectionTooLarge,
    TooManyFields,
    TooManyCases,
    TooDeep,
    TooManyNodes,
    LeafTooLarge,
    InvalidPrimitiveLeaf(PrimitiveInfoRefusal),
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
    /// Nominal semantic identity over one exact finite representation.
    Nominal {
        schema: &'a KindId,
        representation: &'a StructuredInfoType,
    },
    Collection {
        element: &'a StructuredInfoType,
        length: u16,
    },
    Sequence {
        element: &'a StructuredInfoType,
        minimum_items: u16,
        maximum_items: u16,
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
    Nominal {
        schema: KindId,
        representation: alloc::boxed::Box<StructuredInfoType>,
    },
    Collection {
        element: alloc::boxed::Box<StructuredInfoType>,
        length: u16,
    },
    Sequence {
        element: alloc::boxed::Box<StructuredInfoType>,
        minimum_items: u16,
        maximum_items: u16,
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
            StructuredInfoTypeNode::Nominal {
                schema,
                representation,
            } => StructuredInfoTypeShape::Nominal {
                schema,
                representation,
            },
            StructuredInfoTypeNode::Collection { element, length } => {
                StructuredInfoTypeShape::Collection {
                    element,
                    length: *length,
                }
            }
            StructuredInfoTypeNode::Sequence {
                element,
                minimum_items,
                maximum_items,
            } => StructuredInfoTypeShape::Sequence {
                element,
                minimum_items: *minimum_items,
                maximum_items: *maximum_items,
            },
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

    /// Gives one exact representation a distinct authored semantic identity.
    pub fn nominal(
        schema: KindId,
        representation: StructuredInfoType,
    ) -> Result<Self, StructuredInfoRefusal> {
        validate_name(schema.as_str())?;
        let value = Self(StructuredInfoTypeNode::Nominal {
            schema,
            representation: alloc::boxed::Box::new(representation),
        });
        value.validate_limits()?;
        Ok(value)
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

    /// A finite variable-length sequence whose actual length is carried by each value.
    pub fn sequence(
        element: StructuredInfoType,
        maximum_items: u16,
    ) -> Result<Self, StructuredInfoRefusal> {
        Self::bounded_sequence(element, 0, maximum_items)
    }

    /// A finite variable-length sequence with exact cardinality bounds.
    pub fn bounded_sequence(
        element: StructuredInfoType,
        minimum_items: u16,
        maximum_items: u16,
    ) -> Result<Self, StructuredInfoRefusal> {
        if maximum_items == 0 {
            return Err(StructuredInfoRefusal::EmptyShape);
        }
        if minimum_items > maximum_items {
            return Err(StructuredInfoRefusal::InvalidCollectionBounds);
        }
        if usize::from(maximum_items) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
            return Err(StructuredInfoRefusal::CollectionTooLarge);
        }
        let value = Self(StructuredInfoTypeNode::Sequence {
            element: alloc::boxed::Box::new(element),
            minimum_items,
            maximum_items,
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

    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self, StructuredInfoRefusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let (value_type, remaining) = decode_type(encoded)?;
        if remaining.is_empty() {
            Ok(value_type)
        } else {
            Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
        }
    }

    /// Validates one canonical value node against this exact type without allocating.
    pub fn validate_canonical_node(&self, encoded: &[u8]) -> Result<(), StructuredInfoRefusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        if validate_value(self, encoded)?.is_empty() {
            Ok(())
        } else {
            Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
        }
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

/// Canonical finite meaning of Conduitese `T?`: exactly `none | some(T)`.
/// The selected case is carried by the ordinary structured-variant encoding;
/// absence is never an empty byte string or an ambient null sentinel.
pub fn optional_info_type(
    value_type: StructuredInfoType,
) -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::variant(
        crate::kind_id("conduit.conduitese.optional.v1"),
        vec![
            StructuredVariantCase::new(
                "none",
                StructuredInfoType::leaf(crate::kind_id(crate::EMPTY_INFO_ID))?,
            )?,
            StructuredVariantCase::new("some", value_type)?,
        ],
    )
}

/// Prepared, allocation-stable encoder for the canonical optional variant.
/// Hosted profiles prepare its full finite buffer before Play.
pub struct PreparedOptionalInfoEncoder {
    value_type: StructuredInfoType,
    value_type_prefix: Vec<u8>,
    none: Vec<u8>,
    some_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl PreparedOptionalInfoEncoder {
    pub fn new(value_type: StructuredInfoType) -> Result<Self, StructuredInfoRefusal> {
        let optional = optional_info_type(value_type.clone())?;
        let none = StructuredInfoValue::variant(
            optional.clone(),
            "none",
            StructuredInfoValue::leaf(
                StructuredInfoType::leaf(crate::kind_id(crate::EMPTY_INFO_ID))?,
                Vec::new(),
            )?,
        )?
        .canonical_bytes()?;
        let mut some_prefix = optional.canonical_bytes()?;
        some_prefix.push(3);
        some_prefix.extend_from_slice(&4_u32.to_le_bytes());
        some_prefix.extend_from_slice(b"some");
        if some_prefix.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            value_type_prefix: value_type.canonical_bytes()?,
            value_type,
            none,
            some_prefix,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub fn encode(&mut self, payload: Option<&[u8]>) -> Result<&[u8], StructuredInfoRefusal> {
        self.output.clear();
        let Some(payload) = payload else {
            self.output.extend_from_slice(&self.none);
            return Ok(&self.output);
        };
        self.output.extend_from_slice(&self.some_prefix);
        match self.value_type.shape() {
            StructuredInfoTypeShape::Leaf(kind) => {
                validate_primitive_info(kind.as_str(), payload)
                    .map_err(StructuredInfoRefusal::InvalidPrimitiveLeaf)?;
                let encoded_len = self
                    .some_prefix
                    .len()
                    .checked_add(1 + core::mem::size_of::<u32>())
                    .and_then(|len| len.checked_add(payload.len()))
                    .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
                if encoded_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
                    return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
                }
                self.output.push(0);
                self.output
                    .extend_from_slice(&(payload.len() as u32).to_le_bytes());
                self.output.extend_from_slice(payload);
            }
            _ => {
                let node = payload
                    .strip_prefix(self.value_type_prefix.as_slice())
                    .ok_or(StructuredInfoRefusal::WrongType)?;
                let validated = validate_canonical_structured_value(payload)
                    .map_err(|_| StructuredInfoRefusal::WrongType)?;
                if validated.type_bytes() != self.value_type_prefix.as_slice() {
                    return Err(StructuredInfoRefusal::WrongType);
                }
                let encoded_len = self
                    .some_prefix
                    .len()
                    .checked_add(node.len())
                    .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
                if encoded_len > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
                    return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
                }
                self.output.extend_from_slice(node);
            }
        }
        Ok(&self.output)
    }

    pub fn capacity(&self) -> usize {
        self.output.capacity()
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
        let StructuredInfoTypeNode::Leaf(kind) = &value_type.0 else {
            unreachable!("leaf shape checked above")
        };
        validate_primitive_info(kind.as_str(), &canonical_value)
            .map_err(StructuredInfoRefusal::InvalidPrimitiveLeaf)?;
        Self::finish(value_type, StructuredInfoValueNode::Leaf(canonical_value))
    }

    /// Retags a value of the exact declared representation with its nominal Type.
    pub fn nominal(
        value_type: StructuredInfoType,
        representation: StructuredInfoValue,
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeNode::Nominal {
            representation: expected,
            ..
        } = &value_type.0
        else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        if representation.value_type != **expected {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Self::finish(value_type, representation.node)
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

    pub fn sequence(
        value_type: StructuredInfoType,
        values: Vec<StructuredInfoValue>,
    ) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeNode::Sequence {
            element,
            minimum_items,
            maximum_items,
        } = &value_type.0
        else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        if values.len() < usize::from(*minimum_items) || values.len() > usize::from(*maximum_items)
        {
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
