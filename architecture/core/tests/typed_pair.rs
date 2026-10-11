//! Exact structured pairing retains schemas rather than opaque leaf wrappers.
use conduit_core::*;

fn packet(field: &str, value: u64) -> (StructuredInfoType, Vec<u8>) {
    let byte = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("type/Packet@1"),
        vec![StructuredFieldType::new(field, byte.clone()).unwrap()],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            field,
            StructuredInfoValue::leaf(byte, value.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (ty, value)
}

#[test]
fn structured_and_empty_primitive_members_retain_exact_nested_views() {
    let (ty, bytes) = packet("count", 19);
    let mut encoder = PreparedTypedTuplePairEncoder::new(
        ty.clone(),
        bytes.len() as u32,
        StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
        0,
    )
    .unwrap();
    let expected = encoder.value_type().canonical_bytes().unwrap();
    for _ in 0..32 {
        let encoded = encoder.encode(&bytes, &[]).unwrap();
        let view = validate_canonical_structured_value(encoded).unwrap();
        assert_eq!(view.type_bytes(), expected);
        let left = view.record_field("item-00000").unwrap().unwrap();
        assert_eq!(left.type_bytes(), ty.canonical_bytes().unwrap());
        assert_eq!(
            left.record_field("count")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            19_u64.to_le_bytes()
        );
        assert!(view
            .record_field("item-00001")
            .unwrap()
            .unwrap()
            .primitive_bytes(EMPTY_INFO_ID)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn wrong_structural_schema_and_truncation_refuse_before_replacing_output() {
    let (ty, bytes) = packet("count", 19);
    let (wrong_ty, forged) = packet("other", 19);
    assert_ne!(
        ty.profile().unwrap().value_kind(),
        wrong_ty.profile().unwrap().value_kind()
    );
    let mut encoder = PreparedTypedTuplePairEncoder::new(
        ty,
        bytes.len() as u32,
        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        1,
    )
    .unwrap();
    let previous = encoder.encode(&bytes, &[1]).unwrap().to_vec();
    assert_eq!(
        encoder.encode(&forged, &[1]),
        Err(StructuredInfoRefusal::WrongType)
    );
    assert_eq!(encoder.encoded(), previous);
    for end in 0..bytes.len() {
        assert!(encoder.encode(&bytes[..end], &[1]).is_err());
        assert_eq!(encoder.encoded(), previous);
    }
    assert!(encoder.encode(&bytes, &[2]).is_err());
    assert_eq!(encoder.encoded(), previous);
}

#[test]
fn declared_envelopes_refuse_oversize_and_keep_nominal_identity() {
    let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let nominal = StructuredInfoType::nominal(kind_id("type/Generation@1"), leaf.clone()).unwrap();
    let value = StructuredInfoValue::nominal(
        nominal.clone(),
        StructuredInfoValue::leaf(leaf.clone(), 3_u64.to_le_bytes().to_vec()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(matches!(
        PreparedTypedTuplePairEncoder::new(leaf.clone(), u32::MAX, leaf.clone(), 8),
        Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
    ));
    let mut encoder =
        PreparedTypedTuplePairEncoder::new(nominal.clone(), value.len() as u32, leaf.clone(), 8)
            .unwrap();
    let output = encoder.encode(&value, &4_u64.to_le_bytes()).unwrap();
    let view = validate_canonical_structured_value(output).unwrap();
    assert_eq!(
        view.record_field("item-00000")
            .unwrap()
            .unwrap()
            .type_bytes(),
        nominal.canonical_bytes().unwrap()
    );
    assert!(encoder
        .encode(&3_u64.to_le_bytes(), &4_u64.to_le_bytes())
        .is_err());
    assert!(encoder.encode(&value, &[0; 9]).is_err());
}

#[test]
fn combined_traversal_is_admitted_before_accepting_individually_valid_members() {
    let unit = StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap();
    let page = StructuredInfoType::collection(unit, Some(1024)).unwrap();
    let packet = StructuredInfoType::record(
        kind_id("type/Paged@1"),
        (0..8)
            .map(|index| StructuredFieldType::new(format!("page-{index}"), page.clone()).unwrap())
            .collect(),
    )
    .unwrap();
    // Each member allows 8,201 value nodes, but their pair needs 16,403.
    assert!(packet.canonical_bytes().is_ok());
    assert!(matches!(
        PreparedTypedTuplePairEncoder::new(packet.clone(), 100_000, packet, 100_000),
        Err(StructuredInfoRefusal::TooManyNodes)
    ));
}
