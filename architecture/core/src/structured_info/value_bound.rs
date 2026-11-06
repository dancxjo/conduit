//! Checked canonical value envelopes from finite Types and leaf-owned bounds.
use super::{
    canonical_length, StructuredInfoType, StructuredInfoTypeShape as Node,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use crate::KindId;

impl StructuredInfoType {
    /// Upper bound including the complete canonical Type prefix and value node.
    /// The leaf owner supplies each payload bound; this does not infer one from
    /// observed values, a transport ceiling, or a semantic name. Unknown leaf
    /// bounds, arithmetic overflow and an excessive envelope return `None`.
    /// The result is conditional on those leaf envelopes being enforced.
    /// No allocation, value construction, grant or runtime admission occurs.
    pub fn canonical_value_upper_bound(
        &self,
        mut leaf_payload_bytes: impl FnMut(&KindId) -> Option<usize>,
    ) -> Option<usize> {
        add(
            canonical_length::encoded_length(self).ok()?,
            node_bound(self, &mut leaf_payload_bytes)?,
        )
    }
}

fn add(left: usize, right: usize) -> Option<usize> {
    left.checked_add(right)
        .filter(|n| *n <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
}
fn node_bound(
    value: &StructuredInfoType,
    leaf: &mut impl FnMut(&KindId) -> Option<usize>,
) -> Option<usize> {
    match value.shape() {
        Node::Leaf(kind) => add(5, leaf(kind)?),
        // A nominal value reuses its representation's value node; only the
        // Type prefix carries nominal identity.
        Node::Nominal { representation, .. } => node_bound(representation, leaf),
        Node::Collection { element, length } => add(
            5,
            node_bound(element, leaf)?.checked_mul(usize::from(length))?,
        ),
        Node::Sequence {
            element,
            maximum_items,
            ..
        } => add(
            5,
            node_bound(element, leaf)?.checked_mul(usize::from(maximum_items))?,
        ),
        Node::Record { fields, .. } => {
            let mut total = 5;
            for field in fields {
                total = add(
                    total,
                    add(
                        add(4, field.name.len())?,
                        node_bound(&field.value_type, leaf)?,
                    )?,
                )?;
            }
            Some(total)
        }
        Node::Variant { cases, .. } => {
            let mut maximum = 0;
            for case in cases {
                maximum = maximum.max(add(
                    add(5, case.tag.len())?,
                    node_bound(&case.payload_type, leaf)?,
                )?);
            }
            Some(maximum)
        }
    }
}
