use super::*;
use crate::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue, StructuredVariantCase,
};

#[test]
fn borrowed_depth_exhaustion_remains_distinct_from_malformed_encoding() {
    let mut encoded = Vec::new();
    for _ in 0..super::super::MAXIMUM_STRUCTURED_INFO_DEPTH {
        encoded.push(5); // nominal Type
        encoded.extend_from_slice(&1_u32.to_le_bytes());
        encoded.push(b'n');
    }
    encoded.extend_from_slice(
        &StructuredInfoType::leaf(kind_id("value/u8"))
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    );
    encoded.push(0); // one canonical leaf value
    encoded.extend_from_slice(&1_u32.to_le_bytes());
    encoded.push(7);
    assert_eq!(
        validate_canonical_structured_value(&encoded),
        Err(Refusal::TooDeep)
    );
}
use alloc::vec;

#[test]
fn prepared_composition_matches_owned_canonical_values_without_growth() {
    let leaf_type = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("test/record"),
        vec![StructuredFieldType::new("count", leaf_type.clone()).unwrap()],
    )
    .unwrap();
    let variant_type = StructuredInfoType::variant(
        kind_id("test/variant"),
        vec![StructuredVariantCase::new("ready", record_type.clone()).unwrap()],
    )
    .unwrap();
    let mut leaf = PreparedStructuredComposer::new(&leaf_type, 64).unwrap();
    let mut record = PreparedStructuredComposer::new(&record_type, 256).unwrap();
    let mut variant = PreparedStructuredComposer::new(&variant_type, 512).unwrap();
    let capacities = [
        leaf.output.capacity(),
        record.output.capacity(),
        variant.output.capacity(),
    ];
    for count in 0..1000_u16 {
        let encoded = leaf.leaf(&count.to_le_bytes()).unwrap();
        let member = validate_canonical_structured_value(encoded).unwrap();
        assert_eq!(
            member.primitive_bytes("value/u16").unwrap(),
            count.to_le_bytes()
        );
        assert_eq!(member.primitive_bytes("value/i16"), Err(Refusal::WrongType));
        let encoded = record.record(&[member]).unwrap();
        let view = validate_canonical_structured_value(encoded).unwrap();
        assert_eq!(
            view.record_field("count")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u16")
                .unwrap(),
            count.to_le_bytes()
        );
        assert_eq!(view.record_field("absent").unwrap(), None);
        let encoded = variant.variant("ready", view).unwrap();
        let expected = StructuredInfoValue::variant(
            variant_type.clone(),
            "ready",
            StructuredInfoValue::record(
                record_type.clone(),
                vec![StructuredFieldValue::new(
                    "count",
                    StructuredInfoValue::leaf(leaf_type.clone(), count.to_le_bytes().to_vec())
                        .unwrap(),
                )
                .unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(encoded, expected.canonical_bytes().unwrap());
        assert_eq!(
            [
                leaf.output.capacity(),
                record.output.capacity(),
                variant.output.capacity()
            ],
            capacities
        );
    }
}

#[test]
fn composed_envelope_identity_and_aggregate_node_limits_are_enforced() {
    let ty = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let encoded = StructuredInfoValue::leaf(ty.clone(), vec![1, 0])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let view = validate_canonical_structured_value(&encoded).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("test/record"),
        vec![StructuredFieldType::new("count", ty).unwrap()],
    )
    .unwrap();
    let mut record = PreparedStructuredComposer::new(
        &record_type,
        record_type.canonical_bytes().unwrap().len() + 5,
    )
    .unwrap();
    assert_eq!(
        record.record(&[view]),
        Err(Refusal::CanonicalEncodingTooLarge)
    );
    assert_eq!(record.record(&[]), Err(Refusal::WrongRecordFields));
    let wrong = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/i16")).unwrap(),
        vec![1, 0],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert_eq!(
        record.record(&[validate_canonical_structured_value(&wrong).unwrap()]),
        Err(Refusal::WrongType)
    );

    let mut sequence =
        crate::PreparedLeafSequenceEncoder::new(kind_id("value/u8"), 1, 1024).unwrap();
    let sequence_type = sequence.value_type().unwrap();
    let encoded = sequence.encode([&[1][..]; 1024].into_iter()).unwrap();
    let view = validate_canonical_structured_value(encoded).unwrap();
    let fields = (0..17)
        .map(|i| {
            StructuredFieldType::new(alloc::format!("page-{i:02}"), sequence_type.clone()).unwrap()
        })
        .collect();
    let record_type = StructuredInfoType::record(kind_id("test/pages"), fields).unwrap();
    let mut record =
        PreparedStructuredComposer::new(&record_type, MAXIMUM_STRUCTURED_CANONICAL_BYTES).unwrap();
    let capacity = record.output.capacity();
    assert_eq!(record.record(&[view; 17]), Err(Refusal::TooManyNodes));
    assert_eq!(record.output.capacity(), capacity);
}
