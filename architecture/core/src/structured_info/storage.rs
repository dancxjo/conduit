//! Requested heap storage owned by one exact structured Type.
use super::{StructuredInfoType, StructuredInfoTypeNode};
use core::mem::size_of;

impl StructuredInfoType {
    /// Counts requested heap bytes, including vector spare capacity and boxed
    /// child Types. The root value itself and allocator bookkeeping are excluded.
    /// Saturation conservatively refuses any smaller admission ceiling.
    pub fn owned_heap_bytes(&self) -> usize {
        match &self.0 {
            StructuredInfoTypeNode::Leaf(kind) => kind.0.capacity(),
            StructuredInfoTypeNode::Nominal {
                schema,
                representation,
            } => schema
                .0
                .capacity()
                .saturating_add(size_of::<Self>())
                .saturating_add(representation.owned_heap_bytes()),
            StructuredInfoTypeNode::Collection { element, .. }
            | StructuredInfoTypeNode::Sequence { element, .. } => {
                size_of::<Self>().saturating_add(element.owned_heap_bytes())
            }
            StructuredInfoTypeNode::Record { schema, fields } => fields.iter().fold(
                schema.0.capacity().saturating_add(
                    fields
                        .capacity()
                        .saturating_mul(size_of::<super::StructuredFieldType>()),
                ),
                |total, field| {
                    total
                        .saturating_add(field.name.capacity())
                        .saturating_add(field.value_type.owned_heap_bytes())
                },
            ),
            StructuredInfoTypeNode::Variant { schema, cases } => cases.iter().fold(
                schema.0.capacity().saturating_add(
                    cases
                        .capacity()
                        .saturating_mul(size_of::<super::StructuredVariantCase>()),
                ),
                |total, case| {
                    total
                        .saturating_add(case.tag.capacity())
                        .saturating_add(case.payload_type.owned_heap_bytes())
                },
            ),
        }
    }
}

impl StructuredInfoType {
    /// Exact canonical Type prefix length without allocating an encoded buffer.
    pub fn canonical_byte_length(&self) -> Result<usize, super::StructuredInfoRefusal> {
        let length = type_length(self)?;
        if length > super::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(super::StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(length)
    }
}
fn add(a: usize, b: usize) -> Result<usize, super::StructuredInfoRefusal> {
    a.checked_add(b)
        .ok_or(super::StructuredInfoRefusal::CanonicalEncodingTooLarge)
}
fn text_length(text: &str) -> Result<usize, super::StructuredInfoRefusal> {
    add(4, text.len())
}
fn type_length(ty: &StructuredInfoType) -> Result<usize, super::StructuredInfoRefusal> {
    match &ty.0 {
        StructuredInfoTypeNode::Leaf(kind) => add(1, text_length(kind.as_str())?),
        StructuredInfoTypeNode::Nominal {
            schema,
            representation,
        } => add(
            add(1, text_length(schema.as_str())?)?,
            type_length(representation)?,
        ),
        StructuredInfoTypeNode::Collection { element, .. } => add(3, type_length(element)?),
        StructuredInfoTypeNode::Sequence { element, .. } => add(5, type_length(element)?),
        StructuredInfoTypeNode::Record { schema, fields } => {
            fields
                .iter()
                .try_fold(add(5, text_length(schema.as_str())?)?, |total, field| {
                    add(
                        add(total, text_length(&field.name)?)?,
                        type_length(&field.value_type)?,
                    )
                })
        }
        StructuredInfoTypeNode::Variant { schema, cases } => {
            cases
                .iter()
                .try_fold(add(5, text_length(schema.as_str())?)?, |total, case| {
                    add(
                        add(total, text_length(&case.tag)?)?,
                        type_length(&case.payload_type)?,
                    )
                })
        }
    }
}
