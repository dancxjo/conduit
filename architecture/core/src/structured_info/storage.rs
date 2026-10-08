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
