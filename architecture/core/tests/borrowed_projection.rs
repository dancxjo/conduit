use conduit_core::{
    kind_id, validate_canonical_structured_value, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, StructuredVariantCase,
};
#[test]
fn borrowed_sequence_and_case_views_preserve_nominal_identity_and_actual_length() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
    let word = StructuredInfoType::nominal(kind_id("type/Word@1"), leaf.clone()).unwrap();
    let sequence = StructuredInfoType::sequence(word.clone(), 4).unwrap();
    let packet = StructuredInfoType::record(
        kind_id("type/Packet@1"),
        vec![StructuredFieldType::new("words", sequence.clone()).unwrap()],
    )
    .unwrap();
    let reply = StructuredInfoType::variant(
        kind_id("type/Reply@1"),
        vec![
            StructuredVariantCase::new("ready", packet.clone()).unwrap(),
            StructuredVariantCase::new(
                "lost",
                StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    for count in [0, 2, 4] {
        let words = StructuredInfoValue::sequence(
            sequence.clone(),
            (0..count)
                .map(|index| {
                    StructuredInfoValue::nominal(
                        word.clone(),
                        StructuredInfoValue::leaf(
                            leaf.clone(),
                            (index as u16).to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let packet = StructuredInfoValue::record(
            packet.clone(),
            vec![StructuredFieldValue::new("words", words).unwrap()],
        )
        .unwrap();
        let value = StructuredInfoValue::variant(reply.clone(), "ready", packet)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let borrowed = validate_canonical_structured_value(&value).unwrap();
        assert!(borrowed.variant_payload("lost").unwrap().is_none());
        assert!(borrowed.variant_payload("missing").unwrap().is_none());
        let words = borrowed
            .variant_payload("ready")
            .unwrap()
            .unwrap()
            .record_field("words")
            .unwrap()
            .unwrap();
        assert_eq!(words.collection_length().unwrap(), count);
        for index in 0..count {
            let selected = words.collection_index(index as u16).unwrap().unwrap();
            assert_eq!(selected.type_bytes(), word.canonical_bytes().unwrap());
            assert!(selected.primitive_bytes("value/u16").is_err());
            assert_eq!(&selected.value_node()[5..], &(index as u16).to_le_bytes());
        }
        assert!(words.collection_index(count as u16).unwrap().is_none());
        assert!(words.variant_payload("ready").is_err());
    }
}
#[test]
fn truncated_or_forged_values_cannot_produce_a_borrowed_view() {
    let byte = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let array = StructuredInfoType::collection(byte.clone(), Some(2)).unwrap();
    let value = StructuredInfoValue::collection(
        array,
        vec![
            StructuredInfoValue::leaf(byte.clone(), vec![1]).unwrap(),
            StructuredInfoValue::leaf(byte, vec![2]).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    for length in 0..value.len() {
        assert!(validate_canonical_structured_value(&value[..length]).is_err());
    }
    let mut forged = value.clone();
    let count_offset = validate_canonical_structured_value(&value)
        .unwrap()
        .type_bytes()
        .len()
        + 1;
    forged[count_offset..count_offset + 4].copy_from_slice(&3_u32.to_le_bytes());
    assert!(validate_canonical_structured_value(&forged).is_err());
    let borrowed = validate_canonical_structured_value(&value).unwrap();
    assert_eq!(
        borrowed
            .collection_index(1)
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u8")
            .unwrap(),
        &[2]
    );
}
