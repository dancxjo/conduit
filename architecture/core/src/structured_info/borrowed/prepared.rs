//! Prepared validation with an immutable encoded schema, including flash data.
use super::*;

/// Borrows one exact canonical schema instead of decoding or owning a Type tree.
/// This validates representation and primitive leaves, not native-law witnesses
/// or execution authority. A Seed must retain those obligations independently.
#[derive(Clone, Copy)]
pub struct BorrowedStructuredValueValidator<'schema> {
    type_bytes: &'schema [u8],
    maximum_bytes: usize,
}
impl<'schema> BorrowedStructuredValueValidator<'schema> {
    /// Bounded schema validation only; no allocation and no Source decoding.
    pub fn new(
        type_bytes: &'schema [u8],
        maximum_bytes: usize,
    ) -> Result<Self, StructuredInfoRefusal> {
        if maximum_bytes == 0
            || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || type_bytes.len() >= maximum_bytes
        {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        let (_, remaining) = split_type(type_bytes, 1, &mut nodes)?;
        if !remaining.is_empty() {
            return Err(malformed());
        }
        Ok(Self {
            type_bytes,
            maximum_bytes,
        })
    }
    /// Exact existing structured profile identity for this validated immutable
    /// schema. No Type tree, KindId allocation, native authority or value is made.
    pub fn matches_profile(&self, identity: &str) -> bool {
        super::super::profile::matches_canonical_schema(self.type_bytes, identity)
    }

    pub const fn maximum_bytes(&self) -> usize {
        self.maximum_bytes
    }
    pub const fn type_bytes(&self) -> &'schema [u8] {
        self.type_bytes
    }

    /// Validate the whole self-describing value against the prepared exact Type.
    pub fn validate<'value>(
        &self,
        encoded: &'value [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'value>, StructuredInfoRefusal>
    where
        'schema: 'value,
    {
        if encoded.len() > self.maximum_bytes {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let node = encoded
            .strip_prefix(self.type_bytes)
            .ok_or(StructuredInfoRefusal::WrongType)?;
        self.validate_node(node)
    }

    /// The returned view borrows schema and node storage, not this small
    /// validator object. Their original lifetimes remain statically required.
    /// Validate a separately stored value node without copying its schema in
    /// front. The combined schema/node extent remains inside the same ceiling.
    pub fn validate_node<'value>(
        &self,
        value_node: &'value [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'value>, StructuredInfoRefusal>
    where
        'schema: 'value,
    {
        if self
            .type_bytes
            .len()
            .checked_add(value_node.len())
            .is_none_or(|n| n > self.maximum_bytes)
        {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let mut value = Cursor::new(value_node);
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        validate_value(self.type_bytes, &mut value, 1, &mut nodes)?;
        if !value.remaining.is_empty() {
            return Err(malformed());
        }
        Ok(ValidatedCanonicalStructuredValue {
            type_bytes: self.type_bytes,
            value_node,
        })
    }
}
