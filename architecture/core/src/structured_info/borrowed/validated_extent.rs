//! Bounded framing traversal of bytes covered by an immutable validation proof.
//! These helpers cannot create a capability from arbitrary bytes. They are used
//! only to delimit children of an existing private validated value capability.
use super::{
    malformed, Cursor, StructuredInfoRefusal, MAXIMUM_STRUCTURED_INFO_DEPTH,
    MAXIMUM_STRUCTURED_INFO_NODES,
};

pub(super) fn split_validated_type(input: &[u8]) -> Result<(&[u8], &[u8]), StructuredInfoRefusal> {
    let mut cursor = Cursor::new(input);
    let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
    type_extent(&mut cursor, 1, &mut nodes)?;
    Ok(input.split_at(input.len() - cursor.remaining.len()))
}

pub(super) fn skip_validated_value(
    cursor: &mut Cursor<'_>,
    nodes: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    value_extent(cursor, 1, nodes)
}

fn enter(depth: usize, nodes: &mut usize) -> Result<(), StructuredInfoRefusal> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
        return Err(StructuredInfoRefusal::TooDeep);
    }
    if *nodes == 0 {
        return Err(StructuredInfoRefusal::TooManyNodes);
    }
    *nodes -= 1;
    Ok(())
}

fn type_extent(
    cursor: &mut Cursor<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    enter(depth, nodes)?;
    match cursor.byte()? {
        0 => {
            cursor.bytes()?;
        }
        1 => {
            cursor.u16()?;
            type_extent(cursor, depth + 1, nodes)?;
        }
        4 => {
            cursor.u16()?;
            cursor.u16()?;
            type_extent(cursor, depth + 1, nodes)?;
        }
        5 => {
            cursor.bytes()?;
            type_extent(cursor, depth + 1, nodes)?;
        }
        2 | 3 => {
            cursor.bytes()?;
            let count = cursor.length()?;
            for _ in 0..count {
                cursor.bytes()?;
                type_extent(cursor, depth + 1, nodes)?;
            }
        }
        _ => return Err(malformed()),
    }
    Ok(())
}

fn value_extent(
    cursor: &mut Cursor<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    enter(depth, nodes)?;
    match cursor.byte()? {
        0 => {
            cursor.bytes()?;
        }
        1 => {
            let count = cursor.length()?;
            for _ in 0..count {
                value_extent(cursor, depth + 1, nodes)?;
            }
        }
        2 => {
            let count = cursor.length()?;
            for _ in 0..count {
                cursor.bytes()?;
                value_extent(cursor, depth + 1, nodes)?;
            }
        }
        3 => {
            cursor.bytes()?;
            value_extent(cursor, depth + 1, nodes)?;
        }
        _ => return Err(malformed()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        kind_id, validate_canonical_structured_value, StructuredFieldType, StructuredFieldValue,
        StructuredInfoType as T, StructuredInfoTypeShape as TS, StructuredInfoValue as V,
        StructuredInfoValueShape as VS, StructuredVariantCase,
    };
    use alloc::vec;

    fn compare(view: super::super::ValidatedCanonicalStructuredValue<'_>, value: &V) {
        let bytes = value.canonical_bytes().unwrap();
        assert_eq!(value.canonical_byte_length().unwrap(), bytes.len());
        let bounded = value.canonical_bytes_with_limit(bytes.len()).unwrap();
        assert_eq!(bounded, bytes);
        assert_eq!(bounded.capacity(), bytes.len());
        assert_eq!(
            value.canonical_bytes_with_limit(bytes.len() - 1),
            Err(crate::StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
        let expected = validate_canonical_structured_value(&bytes).unwrap();
        assert_eq!(view, expected);
        if let TS::Nominal { representation, .. } = value.value_type().shape() {
            let child = view.nominal_representation().unwrap();
            let mut framed = representation.canonical_bytes().unwrap();
            framed.extend_from_slice(child.value_node());
            compare(child, &V::from_canonical_bytes(&framed).unwrap());
            return;
        }
        match value.shape() {
            VS::Leaf(_) => {
                assert_eq!(view.primitive().unwrap().1, expected.primitive().unwrap().1);
            }
            VS::Record(fields) => {
                for field in fields {
                    compare(
                        view.record_field(field.name()).unwrap().unwrap(),
                        field.value(),
                    );
                }
                assert_eq!(view.record_field("missing").unwrap(), None);
            }
            VS::Collection(values) => {
                let mut elements = view.collection_elements().unwrap();
                for (index, child) in values.iter().enumerate() {
                    assert_eq!(elements.len(), values.len() - index);
                    compare(elements.next().unwrap().unwrap(), child);
                    compare(view.collection_index(index as u16).unwrap().unwrap(), child);
                }
                assert_eq!(elements.len(), 0);
                assert_eq!(elements.next(), None);
                assert_eq!(elements.next(), None);
                assert_eq!(view.collection_index(values.len() as u16).unwrap(), None);
            }
            VS::Variant { tag, payload } => {
                compare(view.variant_payload(tag).unwrap().unwrap(), payload);
                assert_eq!(view.variant_payload("missing").unwrap(), None);
            }
        }
    }

    #[test]
    fn validated_extent_views_match_owned_children_for_every_shape_and_empty_payload() {
        let octet = T::leaf(kind_id("value/u8")).unwrap();
        let unit = T::leaf(kind_id("value/unit")).unwrap();
        let text = T::leaf(kind_id("value/text")).unwrap();
        let collection = T::collection(octet.clone(), Some(2)).unwrap();
        let sequence = T::sequence(octet.clone(), 3).unwrap();
        let nominal = T::nominal(kind_id("test/nominal"), collection.clone()).unwrap();
        let variant = T::variant(
            kind_id("test/variant"),
            vec![
                StructuredVariantCase::new("active", nominal.clone()).unwrap(),
                StructuredVariantCase::new("empty", unit.clone()).unwrap(),
            ],
        )
        .unwrap();
        let record = T::record(
            kind_id("test/record"),
            vec![
                StructuredFieldType::new("a", unit.clone()).unwrap(),
                StructuredFieldType::new("b", sequence.clone()).unwrap(),
                StructuredFieldType::new("c", variant.clone()).unwrap(),
                StructuredFieldType::new("d", text.clone()).unwrap(),
            ],
        )
        .unwrap();
        let leaf = |n| V::leaf(octet.clone(), vec![n]).unwrap();
        let payload = V::nominal(
            nominal,
            V::collection(collection, vec![leaf(3), leaf(9)]).unwrap(),
        )
        .unwrap();
        for (tag, payload) in [
            ("active", payload),
            ("empty", V::leaf(unit.clone(), vec![]).unwrap()),
        ] {
            let value = V::record(
                record.clone(),
                vec![
                    StructuredFieldValue::new("a", V::leaf(unit.clone(), vec![]).unwrap()).unwrap(),
                    StructuredFieldValue::new(
                        "b",
                        V::sequence(sequence.clone(), vec![leaf(2), leaf(7)]).unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "c",
                        V::variant(variant.clone(), tag, payload).unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "d",
                        V::leaf(text.clone(), "λ最後".as_bytes().to_vec()).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            let outer = T::nominal(kind_id("test/outer"), record.clone()).unwrap();
            let value = V::nominal(outer, value).unwrap();
            let bytes = value.canonical_bytes().unwrap();
            compare(validate_canonical_structured_value(&bytes).unwrap(), &value);
            for end in 0..bytes.len() {
                assert!(validate_canonical_structured_value(&bytes[..end]).is_err());
            }
        }
    }
}
