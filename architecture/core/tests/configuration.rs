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
