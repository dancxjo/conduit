use conduit_core::{
    encode_count, kind_id, StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
};

#[test]
fn structured_configuration_requires_matching_profile_and_canonical_value() {
    let value_type = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let value = StructuredInfoValue::leaf(value_type.clone(), encode_count(7).to_vec()).unwrap();
    let canonical = value.canonical_bytes().unwrap();
    let profile = value_type.profile().unwrap().value_kind().clone();

    assert!(StructuredConfigurationValue::new(profile, canonical.clone()).is_some());
    assert!(
        StructuredConfigurationValue::new(kind_id("structured-info/wrong@1"), canonical).is_none()
    );
    assert!(StructuredConfigurationValue::new(
        value_type.profile().unwrap().value_kind().clone(),
        vec![0xff],
    )
    .is_none());
}

#[test]
fn numeric_configuration_validates_intrinsic_bounds_and_canonical_meaning() {
    use conduit_core::{MathValue, NumericForms, PortableNumericProfile};
    let profile = PortableNumericProfile {
        forms: NumericForms::INTEGER.union(NumericForms::RATIONAL),
        maximum_integer_limbs: 1,
        maximum_symbolic_nodes: 0,
        maximum_symbolic_depth: 1,
        maximum_encoded_bytes: 128,
    };
    let value_type = StructuredInfoType::leaf(kind_id(&profile.info_id().unwrap())).unwrap();
    let value = StructuredInfoValue::leaf(
        value_type.clone(),
        MathValue::rational("1", "3")
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    let canonical = value.canonical_bytes().unwrap();
    let config = StructuredConfigurationValue::new(
        value_type.profile().unwrap().value_kind().clone(),
        canonical,
    )
    .unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(config.canonical_value()).unwrap(),
        value
    );
    let large = MathValue::parse_decimal("10000000000000000000000000000000000000000").unwrap();
    assert!(StructuredInfoValue::leaf(value_type, large.canonical_bytes().unwrap()).is_err());
}
