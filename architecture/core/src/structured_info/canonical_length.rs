//! Exact canonical type size without allocating a discarded serialization.
use super::{
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeNode,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub(super) fn encoded_length(value: &StructuredInfoType) -> Result<usize, StructuredInfoRefusal> {
    let add = |left: usize, right: usize| {
        left.checked_add(right)
            .filter(|total| *total <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)
    };
    match &value.0 {
        StructuredInfoTypeNode::Leaf(kind) => add(5, kind.as_str().len()),
        StructuredInfoTypeNode::Nominal {
            schema,
            representation,
        } => add(
            add(5, schema.as_str().len())?,
            encoded_length(representation)?,
        ),
        StructuredInfoTypeNode::Collection { element, .. } => add(3, encoded_length(element)?),
        StructuredInfoTypeNode::Sequence { element, .. } => add(5, encoded_length(element)?),
        StructuredInfoTypeNode::Record { schema, fields } => {
            let mut length = add(9, schema.as_str().len())?;
            for field in fields {
                length = add(length, add(4, field.name.len())?)?;
                length = add(length, encoded_length(&field.value_type)?)?;
            }
            Ok(length)
        }
        StructuredInfoTypeNode::Variant { schema, cases } => {
            let mut length = add(9, schema.as_str().len())?;
            for case in cases {
                length = add(length, add(4, case.tag.len())?)?;
                length = add(length, encoded_length(&case.payload_type)?)?;
            }
            Ok(length)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, StructuredFieldType, StructuredVariantCase};
    use alloc::{string::String, vec};

    #[test]
    fn size_matches_existing_encoding_for_every_type_node() {
        let leaf = StructuredInfoType::leaf(kind_id("value/boolean")).unwrap();
        // Construct the nodes directly so this comparison remains independent
        // of the length check used by their reviewed constructors.
        let values = vec![
            leaf.clone(),
            StructuredInfoType(StructuredInfoTypeNode::Nominal {
                schema: kind_id("nominal"),
                representation: alloc::boxed::Box::new(leaf.clone()),
            }),
            StructuredInfoType(StructuredInfoTypeNode::Collection {
                element: alloc::boxed::Box::new(leaf.clone()),
                length: 3,
            }),
            StructuredInfoType(StructuredInfoTypeNode::Sequence {
                element: alloc::boxed::Box::new(leaf.clone()),
                minimum_items: 0,
                maximum_items: 3,
            }),
            StructuredInfoType(StructuredInfoTypeNode::Record {
                schema: kind_id("record"),
                fields: vec![StructuredFieldType {
                    name: String::from("é"),
                    value_type: leaf.clone(),
                }],
            }),
            StructuredInfoType(StructuredInfoTypeNode::Variant {
                schema: kind_id("variant"),
                cases: vec![StructuredVariantCase {
                    tag: String::from("some"),
                    payload_type: leaf,
                }],
            }),
        ];
        for value in values {
            let mut bytes = vec![];
            super::super::canonical::encode_type(&value, &mut bytes);
            assert_eq!(encoded_length(&value), Ok(bytes.len()));
            assert_eq!(value.canonical_bytes().unwrap(), bytes);
        }
    }

    #[test]
    fn oversized_type_refuses_before_serialization() {
        let value = StructuredInfoType(StructuredInfoTypeNode::Leaf(kind_id(
            &"x".repeat(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        )));
        assert_eq!(
            encoded_length(&value),
            Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
        assert_eq!(
            value.canonical_bytes(),
            Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
    }
}
