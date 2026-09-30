use conduit_core::*;

#[test]
fn prepared_shape_validation_refuses_wrong_type_truncation_trailing_data_and_capacity() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), InfoBool::TRUE.encode().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let validator = PreparedStructuredValueValidator::new(&ty, value.len()).unwrap();
    validator.validate(&value).unwrap();
    for length in 0..value.len() {
        assert!(validator.validate(&value[..length]).is_err());
    }
    let mut altered = value.clone();
    altered.push(0);
    assert_eq!(
        validator.validate(&altered),
        Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
    );
    let wider = PreparedStructuredValueValidator::new(&ty, 64).unwrap();
    assert_eq!(
        wider.validate(&altered),
        Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
    );
    altered = value;
    altered[5] ^= 1;
    assert_eq!(
        wider.validate(&altered),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn prepared_validation_walks_exact_record_members() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/state@1"),
        vec![StructuredFieldType::new("on", leaf.clone()).unwrap()],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "on",
            StructuredInfoValue::leaf(leaf, InfoBool::FALSE.encode().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let validator = PreparedStructuredValueValidator::new(&ty, 128).unwrap();
    validator.validate(&value).unwrap();
    let mut altered = value;
    let prefix = ty.canonical_bytes().unwrap().len();
    altered[prefix + 9] ^= 1;
    assert_eq!(
        validator.validate(&altered),
        Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
    );
}

#[test]
fn prepared_validation_preserves_nominal_identity_while_walking_its_representation() {
    let representation = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty =
        StructuredInfoType::nominal(kind_id("control/enabled@1"), representation.clone()).unwrap();
    let value = StructuredInfoValue::nominal(
        ty.clone(),
        StructuredInfoValue::leaf(representation, InfoBool::TRUE.encode().to_vec()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let validator = PreparedStructuredValueValidator::new(&ty, 128).unwrap();
    validator.validate(&value).unwrap();

    let other = StructuredInfoType::nominal(
        kind_id("control/armed@1"),
        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        PreparedStructuredValueValidator::new(&other, 128)
            .unwrap()
            .validate(&value),
        Err(StructuredInfoRefusal::WrongType)
    );
}

#[test]
fn prepared_validation_enforces_exact_sequence_cardinality_bounds() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = StructuredInfoType::bounded_sequence(leaf.clone(), 2, 3).unwrap();
    for length in 2..=3 {
        let value = StructuredInfoValue::sequence(
            ty.clone(),
            (0..length)
                .map(|_| {
                    StructuredInfoValue::leaf(leaf.clone(), InfoBool::TRUE.encode().to_vec())
                        .unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        PreparedStructuredValueValidator::new(&ty, 256)
            .unwrap()
            .validate(&value)
            .unwrap();
    }

    let value = StructuredInfoValue::sequence(
        ty.clone(),
        vec![
            StructuredInfoValue::leaf(leaf.clone(), InfoBool::TRUE.encode().to_vec()).unwrap(),
            StructuredInfoValue::leaf(leaf, InfoBool::TRUE.encode().to_vec()).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let validator = PreparedStructuredValueValidator::new(&ty, 256).unwrap();
    let mut below_minimum = value;
    let length = ty.canonical_bytes().unwrap().len() + 1;
    below_minimum[length..length + 4].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        validator.validate(&below_minimum),
        Err(StructuredInfoRefusal::MalformedCanonicalEncoding)
    );
}

#[test]
fn prepared_validation_accepts_nested_collection_and_selected_variant_only() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let collection = StructuredInfoType::collection(leaf.clone(), Some(2)).unwrap();
    let ty = StructuredInfoType::variant(
        kind_id("test/choice@1"),
        vec![StructuredVariantCase::new("items", collection.clone()).unwrap()],
    )
    .unwrap();
    let element = StructuredInfoValue::leaf(leaf, InfoBool::TRUE.encode().to_vec()).unwrap();
    let items =
        StructuredInfoValue::collection(collection, vec![element.clone(), element]).unwrap();
    let value = StructuredInfoValue::variant(ty.clone(), "items", items)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let validator = PreparedStructuredValueValidator::new(&ty, 256).unwrap();
    validator.validate(&value).unwrap();
    let mut altered = value;
    let prefix = ty.canonical_bytes().unwrap().len();
    altered[prefix + 5] = b'x';
    assert_eq!(
        validator.validate(&altered),
        Err(StructuredInfoRefusal::UnknownVariantTag)
    );
}
