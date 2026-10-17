use conduit_core::{
    kind_id, validate_canonical_structured_value, validate_primitive_info, PrimitiveInfoRefusal,
    StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, BYTES_INFO_ID, MAXIMUM_BYTES_INFO_BYTES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};

#[test]
fn native_leaf_envelope_and_primitive_bytes_limits_remain_independent() {
    let native_type = StructuredInfoType::leaf(kind_id("test/native-large@1")).unwrap();
    let native =
        StructuredInfoValue::leaf(native_type.clone(), vec![7; MAXIMUM_STRUCTURED_LEAF_BYTES])
            .unwrap();
    let encoded = native.canonical_bytes().unwrap();
    validate_canonical_structured_value(&encoded).unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&encoded).unwrap(),
        native
    );
    assert_eq!(
        StructuredInfoValue::leaf(
            native_type.clone(),
            vec![0; MAXIMUM_STRUCTURED_LEAF_BYTES + 1]
        ),
        Err(StructuredInfoRefusal::LeafTooLarge),
    );
    let bytes_type = StructuredInfoType::leaf(kind_id(BYTES_INFO_ID)).unwrap();
    assert!(
        StructuredInfoValue::leaf(bytes_type.clone(), vec![0; MAXIMUM_BYTES_INFO_BYTES]).is_ok()
    );
    assert_eq!(
        validate_primitive_info(BYTES_INFO_ID, &vec![0; MAXIMUM_BYTES_INFO_BYTES + 1]),
        Err(PrimitiveInfoRefusal::BytesTooLarge {
            maximum: MAXIMUM_BYTES_INFO_BYTES,
            actual: MAXIMUM_BYTES_INFO_BYTES + 1,
        }),
    );
    let mut forged = bytes_type.canonical_bytes().unwrap();
    forged.extend_from_slice(&encoded[native_type.canonical_bytes().unwrap().len()..]);
    assert!(validate_canonical_structured_value(&forged).is_err());
    assert!(StructuredInfoValue::from_canonical_bytes(&forged).is_err());
}

#[test]
fn aggregate_ceiling_still_refuses_two_maximum_native_leaves() {
    let native_type = StructuredInfoType::leaf(kind_id("test/native-large@1")).unwrap();
    let native =
        StructuredInfoValue::leaf(native_type.clone(), vec![0; MAXIMUM_STRUCTURED_LEAF_BYTES])
            .unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("test/native-large-pair@1"),
        vec![
            StructuredFieldType::new("first", native_type.clone()).unwrap(),
            StructuredFieldType::new("second", native_type).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(
        StructuredInfoValue::record(
            record_type,
            vec![
                StructuredFieldValue::new("first", native.clone()).unwrap(),
                StructuredFieldValue::new("second", native).unwrap(),
            ]
        ),
        Err(StructuredInfoRefusal::CanonicalEncodingTooLarge),
    );
}
