//! Pre-Play preparation followed by allocation-free canonical shape validation.
use super::{
    canonical::Cursor, StructuredInfoRefusal as Refusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};
use alloc::vec::Vec;

/// Retains one checked finite schema and its exact canonical prefix.
/// Leaf payload meaning remains owned by its Kind; this validates the canonical
/// structured envelope, shape and bounds, not a second leaf-language checker.
pub struct PreparedStructuredValueValidator {
    value_type: StructuredInfoType,
    prefix: Vec<u8>,
    maximum_bytes: usize,
}

impl PreparedStructuredValueValidator {
    /// Allocates only during preparation, before Play start.
    pub fn new(value_type: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, Refusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let prefix = value_type.canonical_bytes()?;
        if prefix.len() >= maximum_bytes {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            value_type: value_type.clone(),
            prefix,
            maximum_bytes,
        })
    }

    /// Borrows input and traverses the already checked schema without allocation.
    pub fn validate(&self, input: &[u8]) -> Result<(), Refusal> {
        if input.len() > self.maximum_bytes {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let node = input
            .strip_prefix(self.prefix.as_slice())
            .ok_or(Refusal::WrongType)?;
        let mut cursor = Cursor::new(node);
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        validate_node(&self.value_type, &mut cursor, &mut remaining)?;
        if !cursor.remaining.is_empty() {
            return Err(Refusal::MalformedCanonicalEncoding);
        }
        Ok(())
    }
}

fn expect(actual: bool) -> Result<(), Refusal> {
    actual
        .then_some(())
        .ok_or(Refusal::MalformedCanonicalEncoding)
}

fn validate_node(
    ty: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
    remaining: &mut usize,
) -> Result<(), Refusal> {
    *remaining = remaining.checked_sub(1).ok_or(Refusal::TooManyNodes)?;
    match ty.shape() {
        Shape::Leaf(_) => {
            expect(cursor.byte()? == 0)?;
            if cursor.bytes()?.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(Refusal::LeafTooLarge);
            }
        }
        Shape::Collection { element, length } => {
            expect(cursor.byte()? == 1)?;
            expect(cursor.length()? == usize::from(length))?;
            for _ in 0..length {
                validate_node(element, cursor, remaining)?;
            }
        }
        Shape::Record { fields, .. } => {
            expect(cursor.byte()? == 2)?;
            expect(cursor.length()? == fields.len())?;
            for field in fields {
                expect(cursor.bytes()? == field.name().as_bytes())?;
                validate_node(field.value_type(), cursor, remaining)?;
            }
        }
        Shape::Variant { cases, .. } => {
            expect(cursor.byte()? == 3)?;
            let tag = cursor.bytes()?;
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(Refusal::UnknownVariantTag)?;
            validate_node(case.payload_type(), cursor, remaining)?;
        }
    }
    Ok(())
}
