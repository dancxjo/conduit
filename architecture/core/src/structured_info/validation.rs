//! Pre-Play preparation followed by allocation-free canonical shape validation.
use super::{
    canonical::Cursor, StructuredInfoRefusal as Refusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};
use alloc::vec::Vec;

mod contracts;
pub use contracts::{PreparedStructuredContractValidator, StructuredContractValidationRefusal};

/// Retains one checked finite schema and its exact canonical prefix.
/// Leaf payload meaning remains owned by its kind; this validates the canonical
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

    /// Visits borrowed canonical node bodies after validating the whole shape.
    /// Nominal nodes retain their exact Type even though framing is shared with
    /// the representation. Callers may check independently prepared native laws.
    pub fn visit_nodes<E>(
        &self,
        input: &[u8],
        mut visit: impl FnMut(&StructuredInfoType, &[u8]) -> Result<(), E>,
    ) -> Result<(), StructuredNodeVisitRefusal<E>> {
        self.validate(input)
            .map_err(StructuredNodeVisitRefusal::Structure)?;
        let mut cursor = Cursor::new(&input[self.prefix.len()..]);
        visit_node(&self.value_type, &mut cursor, &mut visit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredNodeVisitRefusal<E> {
    Structure(Refusal),
    Visitor(E),
}

fn visit_node<E>(
    ty: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
    visit: &mut impl FnMut(&StructuredInfoType, &[u8]) -> Result<(), E>,
) -> Result<(), StructuredNodeVisitRefusal<E>> {
    use StructuredNodeVisitRefusal::{Structure, Visitor};
    let start = cursor.remaining;
    match ty.shape() {
        Shape::Nominal { representation, .. } => visit_node(representation, cursor, visit)?,
        Shape::Leaf(_) => {
            let _ = cursor.byte().map_err(Structure)?;
            let _ = cursor.bytes().map_err(Structure)?;
        }
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            let _ = cursor.byte().map_err(Structure)?;
            let length = cursor.length().map_err(Structure)?;
            for _ in 0..length {
                visit_node(element, cursor, visit)?;
            }
        }
        Shape::Record { fields, .. } => {
            let _ = cursor.byte().map_err(Structure)?;
            let _ = cursor.length().map_err(Structure)?;
            for field in fields {
                let _ = cursor.bytes().map_err(Structure)?;
                visit_node(field.value_type(), cursor, visit)?;
            }
        }
        Shape::Variant { cases, .. } => {
            let _ = cursor.byte().map_err(Structure)?;
            let tag = cursor.bytes().map_err(Structure)?;
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(Structure(Refusal::UnknownVariantTag))?;
            visit_node(case.payload_type(), cursor, visit)?;
        }
    }
    visit(ty, &start[..start.len() - cursor.remaining.len()]).map_err(Visitor)
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
        Shape::Nominal { representation, .. } => {
            validate_node(representation, cursor, remaining)?;
        }
        Shape::Leaf(kind) => {
            expect(cursor.byte()? == 0)?;
            let encoded = cursor.bytes()?;
            if encoded.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(Refusal::LeafTooLarge);
            }
            crate::validate_primitive_info(kind.as_str(), encoded)
                .map_err(Refusal::InvalidPrimitiveLeaf)?;
        }
        Shape::Collection { element, length } => {
            expect(cursor.byte()? == 1)?;
            expect(cursor.length()? == usize::from(length))?;
            for _ in 0..length {
                validate_node(element, cursor, remaining)?;
            }
        }
        Shape::Sequence {
            element,
            minimum_items,
            maximum_items,
        } => {
            expect(cursor.byte()? == 1)?;
            let length = cursor.length()?;
            expect(length >= usize::from(minimum_items) && length <= usize::from(maximum_items))?;
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
