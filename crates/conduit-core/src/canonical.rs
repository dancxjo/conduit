//! Canonical descriptor encoding and semantic hashes.
//!
//! The encoder writes directly to a caller-provided sink. It sorts borrowed
//! maps and sets by repeated comparison rather than allocating scratch space,
//! which keeps the portable core allocator-free.

use core::cmp::Ordering;
use core::convert::Infallible;
use core::fmt;

use sha2::{Digest, Sha256};

use crate::Id;

/// Version of the canonical descriptor byte form.
pub const CANONICAL_FORM_VERSION: u8 = 0;

/// Header at the beginning of every canonical descriptor.
pub const CANONICAL_MAGIC: [u8; 4] = [b'C', b'N', b'D', CANONICAL_FORM_VERSION];

/// Domain separator prepended before hashing canonical descriptor bytes.
pub const SEMANTIC_HASH_DOMAIN: &[u8] = b"conduit.semantic-hash\0";

/// Maximum nested collection depth accepted by the portable encoder.
pub const MAX_CANONICAL_DEPTH: u8 = 64;

const TAG_NULL: u8 = 0x00;
const TAG_FALSE: u8 = 0x01;
const TAG_TRUE: u8 = 0x02;
const TAG_INTEGER: u8 = 0x10;
const TAG_BYTES: u8 = 0x20;
const TAG_TEXT: u8 = 0x21;
const TAG_IDENTIFIER: u8 = 0x22;
const TAG_LIST: u8 = 0x30;
const TAG_MAP: u8 = 0x31;
const TAG_SET: u8 = 0x32;

/// A destination for canonical bytes.
///
/// Sinks may stream to a digest, fixed storage, a transport, or hosted
/// allocation without changing the canonical form.
pub trait CanonicalSink {
    /// Sink-specific write failure.
    type Error;

    /// Writes the complete byte slice or returns an error.
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

/// Whether and how a map field participates in semantic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldDisposition<'a> {
    /// The field always participates.
    Semantic,
    /// The field is omitted when canonically equal to this schema default.
    Defaulted(&'a CanonicalValue<'a>),
    /// Prose, labels, spans, layout, and other annotations do not participate.
    Annotation,
}

/// One borrowed descriptor-map field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapField<'a> {
    /// Portable field identifier.
    pub name: Id<'a>,
    /// Abstract descriptor value.
    pub value: CanonicalValue<'a>,
    /// Semantic identity participation selected by the descriptor schema.
    pub disposition: FieldDisposition<'a>,
}

/// Encoding-independent abstract value accepted by canonical form version 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalValue<'a> {
    /// Null.
    Null,
    /// Boolean.
    Boolean(bool),
    /// Signed mathematical integer.
    Integer(i128),
    /// Opaque bytes.
    Bytes(&'a [u8]),
    /// Exact UTF-8 text.
    Text(&'a str),
    /// Validated portable identifier.
    Identifier(Id<'a>),
    /// Ordered values.
    List(&'a [CanonicalValue<'a>]),
    /// Fields sorted by identifier during encoding.
    Map(&'a [MapField<'a>]),
    /// Unique values sorted by their canonical byte order during encoding.
    Set(&'a [CanonicalValue<'a>]),
}

/// A complete descriptor with kind and independently versioned schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDescriptor<'a> {
    /// Descriptor kind, such as `conduit/node-contract`.
    pub kind: Id<'a>,
    /// Exact schema version for that descriptor kind.
    pub schema_version: u32,
    /// Schema-lowered descriptor body.
    pub body: CanonicalValue<'a>,
}

/// Canonicalization or sink failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalError<E> {
    /// A descriptor or field identifier is outside the portable grammar.
    InvalidIdentifier,
    /// Two map fields have the same identifier.
    DuplicateMapKey,
    /// Two set members are canonically equal.
    DuplicateSetValue,
    /// A collection or byte string cannot be represented by the v1 length.
    LengthOverflow,
    /// Nested collections exceed [`MAX_CANONICAL_DEPTH`].
    NestingTooDeep,
    /// The destination rejected bytes.
    Sink(E),
}

impl<E: fmt::Display> fmt::Display for CanonicalError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier => formatter.write_str("invalid canonical identifier"),
            Self::DuplicateMapKey => formatter.write_str("duplicate canonical map key"),
            Self::DuplicateSetValue => formatter.write_str("duplicate canonical set value"),
            Self::LengthOverflow => formatter.write_str("canonical value length exceeds u64"),
            Self::NestingTooDeep => formatter.write_str("canonical value nesting exceeds 64"),
            Self::Sink(error) => write!(formatter, "canonical sink failed: {error}"),
        }
    }
}

/// Algorithm-qualified SHA-256 semantic identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticHash([u8; 32]);

impl SemanticHash {
    /// Constructs a hash from exact digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the SHA-256 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for SemanticHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sha256:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl CanonicalDescriptor<'_> {
    /// Streams the canonical v1 descriptor bytes.
    pub fn write_canonical<S: CanonicalSink>(
        &self,
        sink: &mut S,
    ) -> Result<(), CanonicalError<S::Error>> {
        validate_id(self.kind)?;
        write_bytes(sink, &CANONICAL_MAGIC)?;
        write_id(sink, self.kind)?;
        write_bytes(sink, &self.schema_version.to_be_bytes())?;
        write_value(sink, &self.body, 0)
    }

    /// Computes the domain-separated semantic hash without allocating.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let mut sink = HashSink(Sha256::new());
        sink.write(SEMANTIC_HASH_DOMAIN)
            .map_err(CanonicalError::Sink)?;
        self.write_canonical(&mut sink)?;
        Ok(SemanticHash(sink.0.finalize().into()))
    }
}

/// Hash a descriptor whose body contains ordinary fields plus one set of
/// already-domain-separated semantic hashes.
///
/// This specialized writer lets allocator-free aggregate descriptors encode a
/// caller-owned, variably sized fact set without constructing a recursive
/// `CanonicalValue` scratch tree.
pub(crate) fn semantic_hash_with_hash_set(
    kind: Id<'_>,
    schema_version: u32,
    fields: &[MapField<'_>],
    set_name: Id<'_>,
    hashes: &[SemanticHash],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let mut sink = HashSink(Sha256::new());
    sink.write(SEMANTIC_HASH_DOMAIN)
        .map_err(CanonicalError::Sink)?;
    write_descriptor_with_hash_set(&mut sink, kind, schema_version, fields, set_name, hashes)?;
    Ok(SemanticHash(sink.0.finalize().into()))
}

fn write_descriptor_with_hash_set<S: CanonicalSink>(
    sink: &mut S,
    kind: Id<'_>,
    schema_version: u32,
    fields: &[MapField<'_>],
    set_name: Id<'_>,
    hashes: &[SemanticHash],
) -> Result<(), CanonicalError<S::Error>> {
    validate_id(kind)?;
    validate_id(set_name)?;
    validate_map(fields, 1)?;
    if fields.iter().any(|field| {
        field.name == set_name || !matches!(field.disposition, FieldDisposition::Semantic)
    }) {
        return Err(CanonicalError::DuplicateMapKey);
    }
    for (index, hash) in hashes.iter().enumerate() {
        if hashes[..index].contains(hash) {
            return Err(CanonicalError::DuplicateSetValue);
        }
    }

    write_bytes(sink, &CANONICAL_MAGIC)?;
    write_id(sink, kind)?;
    write_bytes(sink, &schema_version.to_be_bytes())?;
    write_byte(sink, TAG_MAP)?;
    write_length(sink, fields.len() + 1)?;

    for rank in 0..=fields.len() {
        let (selected, selected_is_set) = aggregate_field_at_rank(fields, set_name, rank)
            .ok_or(CanonicalError::LengthOverflow)?;
        write_id(sink, selected)?;
        if selected_is_set {
            write_byte(sink, TAG_SET)?;
            write_length(sink, hashes.len())?;
            for hash_rank in 0..hashes.len() {
                let hash = hash_at_rank(hashes, hash_rank).ok_or(CanonicalError::LengthOverflow)?;
                write_value(sink, &CanonicalValue::Bytes(hash.as_bytes()), 2)?;
            }
        } else {
            let field = fields
                .iter()
                .find(|field| field.name == selected)
                .ok_or(CanonicalError::LengthOverflow)?;
            write_value(sink, &field.value, 1)?;
        }
    }

    Ok(())
}

fn aggregate_field_at_rank<'a>(
    fields: &[MapField<'a>],
    set_name: Id<'a>,
    rank: usize,
) -> Option<(Id<'a>, bool)> {
    let set_rank = fields
        .iter()
        .filter(|field| canonical_id_less(field.name, set_name))
        .count();
    if set_rank == rank {
        return Some((set_name, true));
    }
    fields.iter().find_map(|candidate| {
        let field_rank = fields
            .iter()
            .filter(|field| canonical_id_less(field.name, candidate.name))
            .count()
            + usize::from(canonical_id_less(set_name, candidate.name));
        (field_rank == rank).then_some((candidate.name, false))
    })
}

fn canonical_id_less(left: Id<'_>, right: Id<'_>) -> bool {
    left.as_str()
        .len()
        .cmp(&right.as_str().len())
        .then_with(|| left.as_str().as_bytes().cmp(right.as_str().as_bytes()))
        == Ordering::Less
}

fn hash_at_rank(hashes: &[SemanticHash], rank: usize) -> Option<&SemanticHash> {
    hashes.iter().find(|candidate| {
        hashes
            .iter()
            .filter(|other| other.as_bytes() < candidate.as_bytes())
            .count()
            == rank
    })
}

struct HashSink(Sha256);

impl CanonicalSink for HashSink {
    type Error = Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.update(bytes);
        Ok(())
    }
}

fn write_value<S: CanonicalSink>(
    sink: &mut S,
    value: &CanonicalValue<'_>,
    depth: u8,
) -> Result<(), CanonicalError<S::Error>> {
    match value {
        CanonicalValue::Null => write_byte(sink, TAG_NULL),
        CanonicalValue::Boolean(false) => write_byte(sink, TAG_FALSE),
        CanonicalValue::Boolean(true) => write_byte(sink, TAG_TRUE),
        CanonicalValue::Integer(value) => {
            write_byte(sink, TAG_INTEGER)?;
            write_bytes(sink, &value.to_be_bytes())
        }
        CanonicalValue::Bytes(value) => {
            write_byte(sink, TAG_BYTES)?;
            write_length(sink, value.len())?;
            write_bytes(sink, value)
        }
        CanonicalValue::Text(value) => {
            write_byte(sink, TAG_TEXT)?;
            write_length(sink, value.len())?;
            write_bytes(sink, value.as_bytes())
        }
        CanonicalValue::Identifier(value) => {
            validate_id(*value)?;
            write_id(sink, *value)
        }
        CanonicalValue::List(values) => {
            let child_depth = child_depth(depth)?;
            write_byte(sink, TAG_LIST)?;
            write_length(sink, values.len())?;
            for value in *values {
                write_value(sink, value, child_depth)?;
            }
            Ok(())
        }
        CanonicalValue::Map(fields) => {
            let child_depth = child_depth(depth)?;
            validate_map(fields, child_depth)?;
            write_byte(sink, TAG_MAP)?;
            let field_count = included_field_count(fields, child_depth)?;
            write_length(sink, field_count)?;
            for rank in 0..field_count {
                let field = map_field_at_rank(fields, rank, child_depth)?;
                write_id(sink, field.name)?;
                write_value(sink, &field.value, child_depth)?;
            }
            Ok(())
        }
        CanonicalValue::Set(values) => {
            let child_depth = child_depth(depth)?;
            validate_set(values, child_depth)?;
            write_byte(sink, TAG_SET)?;
            write_length(sink, values.len())?;
            for rank in 0..values.len() {
                let value = set_value_at_rank(values, rank, child_depth)?;
                write_value(sink, value, child_depth)?;
            }
            Ok(())
        }
    }
}

fn write_id<S: CanonicalSink>(sink: &mut S, value: Id<'_>) -> Result<(), CanonicalError<S::Error>> {
    validate_id(value)?;
    write_byte(sink, TAG_IDENTIFIER)?;
    write_length(sink, value.as_str().len())?;
    write_bytes(sink, value.as_str().as_bytes())
}

fn write_byte<S: CanonicalSink>(sink: &mut S, byte: u8) -> Result<(), CanonicalError<S::Error>> {
    write_bytes(sink, &[byte])
}

fn write_length<S: CanonicalSink>(
    sink: &mut S,
    length: usize,
) -> Result<(), CanonicalError<S::Error>> {
    let length = u64::try_from(length).map_err(|_| CanonicalError::LengthOverflow)?;
    write_bytes(sink, &length.to_be_bytes())
}

fn write_bytes<S: CanonicalSink>(
    sink: &mut S,
    bytes: &[u8],
) -> Result<(), CanonicalError<S::Error>> {
    sink.write(bytes).map_err(CanonicalError::Sink)
}

fn child_depth<E>(depth: u8) -> Result<u8, CanonicalError<E>> {
    let depth = depth.checked_add(1).ok_or(CanonicalError::NestingTooDeep)?;
    if depth > MAX_CANONICAL_DEPTH {
        return Err(CanonicalError::NestingTooDeep);
    }
    Ok(depth)
}

fn validate_id<E>(value: Id<'_>) -> Result<(), CanonicalError<E>> {
    Id::new(value.as_str())
        .map(|_| ())
        .map_err(|_| CanonicalError::InvalidIdentifier)
}

fn field_is_included<E>(field: &MapField<'_>, depth: u8) -> Result<bool, CanonicalError<E>> {
    match field.disposition {
        FieldDisposition::Semantic => Ok(true),
        FieldDisposition::Annotation => Ok(false),
        FieldDisposition::Defaulted(default) => {
            Ok(compare_values(&field.value, default, depth)? != Ordering::Equal)
        }
    }
}

fn included_field_count<E>(fields: &[MapField<'_>], depth: u8) -> Result<usize, CanonicalError<E>> {
    let mut count = 0;
    for field in fields {
        if field_is_included(field, depth)? {
            count += 1;
        }
    }
    Ok(count)
}

fn validate_map<E>(fields: &[MapField<'_>], depth: u8) -> Result<(), CanonicalError<E>> {
    for (index, field) in fields.iter().enumerate() {
        validate_id(field.name)?;
        for other in &fields[..index] {
            if field.name == other.name {
                return Err(CanonicalError::DuplicateMapKey);
            }
        }
        if field_is_included(field, depth)? {
            validate_value(&field.value, depth)?;
        }
    }
    Ok(())
}

fn map_field_at_rank<'a, E>(
    fields: &'a [MapField<'a>],
    rank: usize,
    depth: u8,
) -> Result<&'a MapField<'a>, CanonicalError<E>> {
    for candidate in fields {
        if !field_is_included(candidate, depth)? {
            continue;
        }
        let mut preceding = 0;
        for other in fields {
            if field_is_included(other, depth)?
                && compare_ids(other.name, candidate.name)? == Ordering::Less
            {
                preceding += 1;
            }
        }
        if preceding == rank {
            return Ok(candidate);
        }
    }
    Err(CanonicalError::DuplicateMapKey)
}

fn validate_set<E>(values: &[CanonicalValue<'_>], depth: u8) -> Result<(), CanonicalError<E>> {
    for (index, value) in values.iter().enumerate() {
        validate_value(value, depth)?;
        for other in &values[..index] {
            if compare_values(value, other, depth)? == Ordering::Equal {
                return Err(CanonicalError::DuplicateSetValue);
            }
        }
    }
    Ok(())
}

fn set_value_at_rank<'a, E>(
    values: &'a [CanonicalValue<'a>],
    rank: usize,
    depth: u8,
) -> Result<&'a CanonicalValue<'a>, CanonicalError<E>> {
    for candidate in values {
        let mut preceding = 0;
        for other in values {
            if compare_values(other, candidate, depth)? == Ordering::Less {
                preceding += 1;
            }
        }
        if preceding == rank {
            return Ok(candidate);
        }
    }
    Err(CanonicalError::DuplicateSetValue)
}

fn validate_value<E>(value: &CanonicalValue<'_>, depth: u8) -> Result<(), CanonicalError<E>> {
    match value {
        CanonicalValue::Identifier(value) => validate_id(*value),
        CanonicalValue::List(values) => {
            let depth = child_depth(depth)?;
            for value in *values {
                validate_value(value, depth)?;
            }
            Ok(())
        }
        CanonicalValue::Map(fields) => {
            let depth = child_depth(depth)?;
            validate_map(fields, depth)
        }
        CanonicalValue::Set(values) => {
            let depth = child_depth(depth)?;
            validate_set(values, depth)
        }
        _ => Ok(()),
    }
}

fn compare_values<E>(
    left: &CanonicalValue<'_>,
    right: &CanonicalValue<'_>,
    depth: u8,
) -> Result<Ordering, CanonicalError<E>> {
    let rank_order = value_rank(left).cmp(&value_rank(right));
    if rank_order != Ordering::Equal {
        return Ok(rank_order);
    }

    match (left, right) {
        (CanonicalValue::Null, CanonicalValue::Null) => Ok(Ordering::Equal),
        (CanonicalValue::Boolean(left), CanonicalValue::Boolean(right)) => Ok(left.cmp(right)),
        (CanonicalValue::Integer(left), CanonicalValue::Integer(right)) => {
            Ok(left.to_be_bytes().cmp(&right.to_be_bytes()))
        }
        (CanonicalValue::Bytes(left), CanonicalValue::Bytes(right)) => compare_slices(left, right),
        (CanonicalValue::Text(left), CanonicalValue::Text(right)) => {
            compare_slices(left.as_bytes(), right.as_bytes())
        }
        (CanonicalValue::Identifier(left), CanonicalValue::Identifier(right)) => {
            compare_ids(*left, *right)
        }
        (CanonicalValue::List(left), CanonicalValue::List(right)) => {
            let depth = child_depth(depth)?;
            compare_lists(left, right, depth)
        }
        (CanonicalValue::Map(left), CanonicalValue::Map(right)) => {
            let depth = child_depth(depth)?;
            compare_maps(left, right, depth)
        }
        (CanonicalValue::Set(left), CanonicalValue::Set(right)) => {
            let depth = child_depth(depth)?;
            compare_sets(left, right, depth)
        }
        _ => Ok(Ordering::Equal),
    }
}

const fn value_rank(value: &CanonicalValue<'_>) -> u8 {
    match value {
        CanonicalValue::Null => TAG_NULL,
        CanonicalValue::Boolean(false) => TAG_FALSE,
        CanonicalValue::Boolean(true) => TAG_TRUE,
        CanonicalValue::Integer(_) => TAG_INTEGER,
        CanonicalValue::Bytes(_) => TAG_BYTES,
        CanonicalValue::Text(_) => TAG_TEXT,
        CanonicalValue::Identifier(_) => TAG_IDENTIFIER,
        CanonicalValue::List(_) => TAG_LIST,
        CanonicalValue::Map(_) => TAG_MAP,
        CanonicalValue::Set(_) => TAG_SET,
    }
}

fn compare_ids<E>(left: Id<'_>, right: Id<'_>) -> Result<Ordering, CanonicalError<E>> {
    validate_id(left)?;
    validate_id(right)?;
    compare_slices(left.as_str().as_bytes(), right.as_str().as_bytes())
}

fn compare_slices<E>(left: &[u8], right: &[u8]) -> Result<Ordering, CanonicalError<E>> {
    let left_length = u64::try_from(left.len()).map_err(|_| CanonicalError::LengthOverflow)?;
    let right_length = u64::try_from(right.len()).map_err(|_| CanonicalError::LengthOverflow)?;
    Ok(left_length.cmp(&right_length).then_with(|| left.cmp(right)))
}

fn compare_lists<E>(
    left: &[CanonicalValue<'_>],
    right: &[CanonicalValue<'_>],
    depth: u8,
) -> Result<Ordering, CanonicalError<E>> {
    let length = compare_lengths(left.len(), right.len())?;
    if length != Ordering::Equal {
        return Ok(length);
    }
    for (left, right) in left.iter().zip(right) {
        let order = compare_values(left, right, depth)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

fn compare_maps<E>(
    left: &[MapField<'_>],
    right: &[MapField<'_>],
    depth: u8,
) -> Result<Ordering, CanonicalError<E>> {
    validate_map(left, depth)?;
    validate_map(right, depth)?;
    let left_count = included_field_count(left, depth)?;
    let right_count = included_field_count(right, depth)?;
    let length = compare_lengths(left_count, right_count)?;
    if length != Ordering::Equal {
        return Ok(length);
    }
    for rank in 0..left_count {
        let left = map_field_at_rank(left, rank, depth)?;
        let right = map_field_at_rank(right, rank, depth)?;
        let key_order = compare_ids(left.name, right.name)?;
        if key_order != Ordering::Equal {
            return Ok(key_order);
        }
        let value_order = compare_values(&left.value, &right.value, depth)?;
        if value_order != Ordering::Equal {
            return Ok(value_order);
        }
    }
    Ok(Ordering::Equal)
}

fn compare_sets<E>(
    left: &[CanonicalValue<'_>],
    right: &[CanonicalValue<'_>],
    depth: u8,
) -> Result<Ordering, CanonicalError<E>> {
    validate_set(left, depth)?;
    validate_set(right, depth)?;
    let length = compare_lengths(left.len(), right.len())?;
    if length != Ordering::Equal {
        return Ok(length);
    }
    for rank in 0..left.len() {
        let left = set_value_at_rank(left, rank, depth)?;
        let right = set_value_at_rank(right, rank, depth)?;
        let order = compare_values(left, right, depth)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

fn compare_lengths<E>(left: usize, right: usize) -> Result<Ordering, CanonicalError<E>> {
    let left = u64::try_from(left).map_err(|_| CanonicalError::LengthOverflow)?;
    let right = u64::try_from(right).map_err(|_| CanonicalError::LengthOverflow)?;
    Ok(left.cmp(&right))
}

#[cfg(test)]
mod aggregate_tests {
    extern crate std;

    use self::std::vec::Vec;
    use super::*;

    struct VecSink(Vec<u8>);

    impl CanonicalSink for VecSink {
        type Error = Infallible;

        fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn specialized_hash_set_writer_matches_general_canonical_form() {
        let hashes = [
            SemanticHash::from_bytes([2; 32]),
            SemanticHash::from_bytes([1; 32]),
        ];
        let facts = [
            CanonicalValue::Bytes(hashes[0].as_bytes()),
            CanonicalValue::Bytes(hashes[1].as_bytes()),
        ];
        let ordinary = [
            MapField {
                name: Id("source"),
                value: CanonicalValue::Bytes(&[7; 32]),
                disposition: FieldDisposition::Semantic,
            },
            MapField {
                name: Id("created_tick"),
                value: CanonicalValue::Integer(4),
                disposition: FieldDisposition::Semantic,
            },
        ];
        let complete = [
            ordinary[0],
            MapField {
                name: Id("facts"),
                value: CanonicalValue::Set(&facts),
                disposition: FieldDisposition::Semantic,
            },
            ordinary[1],
        ];
        let descriptor = CanonicalDescriptor {
            kind: Id("conduit/execution-plan"),
            schema_version: 0,
            body: CanonicalValue::Map(&complete),
        };
        let expected = descriptor.semantic_hash().unwrap();
        let mut general_bytes = VecSink(Vec::new());
        descriptor.write_canonical(&mut general_bytes).unwrap();
        let mut specialized_bytes = VecSink(Vec::new());
        write_descriptor_with_hash_set(
            &mut specialized_bytes,
            Id("conduit/execution-plan"),
            0,
            &ordinary,
            Id("facts"),
            &hashes,
        )
        .unwrap();
        assert_eq!(specialized_bytes.0, general_bytes.0);
        assert_eq!(
            semantic_hash_with_hash_set(
                Id("conduit/execution-plan"),
                0,
                &ordinary,
                Id("facts"),
                &hashes,
            ),
            Ok(expected)
        );
    }
}
