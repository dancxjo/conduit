use conduit_core::*;

#[test]
fn primitive_registry_is_exact_and_has_no_boolean_alias() {
    let registered = [
        (UNIT_INFO_ID, PrimitiveInfoKind::Unit),
        (BOOL_INFO_ID, PrimitiveInfoKind::Bool),
        (COUNT_INFO_ID, PrimitiveInfoKind::Count),
        (SCALAR_INFO_ID, PrimitiveInfoKind::Scalar),
        (TEXT_INFO_ID, PrimitiveInfoKind::Text),
        (BYTES_INFO_ID, PrimitiveInfoKind::Bytes),
        (QUANTITY_INFO_ID, PrimitiveInfoKind::Quantity),
    ];
    for (identity, kind) in registered {
        assert_eq!(primitive_info_kind(identity), Some(kind));
    }
    assert_eq!(primitive_info_kind("value/boolean"), None);
    assert_eq!(primitive_info_kind("domain/leaf@1"), None);
}

#[test]
fn every_primitive_has_one_canonical_leaf_contract() {
    assert_eq!(validate_primitive_info(UNIT_INFO_ID, &[]), Ok(()));
    assert!(validate_primitive_info(UNIT_INFO_ID, &[0]).is_err());

    for value in [InfoBool::FALSE, InfoBool::TRUE] {
        assert_eq!(
            validate_primitive_info(BOOL_INFO_ID, &value.encode()),
            Ok(())
        );
    }
    assert!(validate_primitive_info(BOOL_INFO_ID, b"true").is_err());

    let count = encode_count(0x0102_0304_0506_0708);
    assert_eq!(count, [8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(decode_count(&count), Ok(0x0102_0304_0506_0708));
    assert!(validate_primitive_info(COUNT_INFO_ID, b"42").is_err());

    assert_eq!(
        validate_primitive_info(SCALAR_INFO_ID, &Scalar::ONE.encode()),
        Ok(())
    );
    assert!(validate_primitive_info(SCALAR_INFO_ID, b"1.0").is_err());
    assert_eq!(
        validate_primitive_info(TEXT_INFO_ID, "hello".as_bytes()),
        Ok(())
    );
    assert!(validate_primitive_info(TEXT_INFO_ID, &[0xff]).is_err());
    assert_eq!(validate_primitive_info(BYTES_INFO_ID, &[0xff, 0]), Ok(()));
    assert_eq!(
        validate_primitive_info(
            QUANTITY_INFO_ID,
            &Quantity::new(-17, QuantityUnit::Millivolt).encode(),
        ),
        Ok(())
    );
    assert_eq!(validate_primitive_info("domain/leaf@1", b"owned"), Ok(()));
}

#[test]
fn nested_primitive_record_is_rejected_when_a_leaf_is_not_canonical() {
    let leaf = |identity| StructuredInfoType::leaf(kind_id(identity)).unwrap();
    let fields = [
        ("bool", leaf(BOOL_INFO_ID), InfoBool::TRUE.encode().to_vec()),
        ("count", leaf(COUNT_INFO_ID), encode_count(7).to_vec()),
        (
            "quantity",
            leaf(QUANTITY_INFO_ID),
            Quantity::new(2, QuantityUnit::Second).encode().to_vec(),
        ),
        (
            "scalar",
            leaf(SCALAR_INFO_ID),
            Scalar::ONE.encode().to_vec(),
        ),
        ("text", leaf(TEXT_INFO_ID), b"seven".to_vec()),
        ("unit", leaf(UNIT_INFO_ID), Vec::new()),
    ];
    let ty = StructuredInfoType::record(
        kind_id("test/primitive-record@1"),
        fields
            .iter()
            .map(|(name, ty, _)| StructuredFieldType::new(*name, ty.clone()).unwrap())
            .collect(),
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(name, ty, bytes)| {
                StructuredFieldValue::new(name, StructuredInfoValue::leaf(ty, bytes).unwrap())
                    .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let canonical = value.canonical_bytes().unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&canonical),
        Ok(value)
    );
    PreparedStructuredValueValidator::new(&ty, canonical.len())
        .unwrap()
        .validate(&canonical)
        .unwrap();

    assert!(StructuredInfoValue::leaf(leaf(COUNT_INFO_ID), b"7".to_vec()).is_err());
    assert!(StructuredInfoValue::leaf(leaf(UNIT_INFO_ID), vec![0]).is_err());
}
