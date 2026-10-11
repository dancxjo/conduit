//! Intrinsic Dimension configuration identity must preserve nominal custody.
use conduit_core::{
    kind_id, ConfigurationValue, Dimension, StructuredConfigurationValue, StructuredInfoType,
    StructuredInfoValue, COUNT_INFO_ID, DIMENSION_INFO_ID,
};

fn configured(value: StructuredInfoValue) -> ConfigurationValue {
    let profile = value.value_type().profile().unwrap().value_kind().clone();
    ConfigurationValue::Structured(
        StructuredConfigurationValue::new(profile, value.canonical_bytes().unwrap()).unwrap(),
    )
}

#[test]
fn dimension_configuration_preserves_intrinsic_and_nominal_identities() {
    let dimension_type = StructuredInfoType::leaf(kind_id(DIMENSION_INFO_ID)).unwrap();
    let dimension = StructuredInfoValue::leaf(
        dimension_type.clone(),
        Dimension::basis("travel").unwrap().encode().to_vec(),
    )
    .unwrap();
    assert_eq!(
        configured(dimension.clone()).semantic_kind(),
        kind_id(DIMENSION_INFO_ID)
    );

    let nominal_type =
        StructuredInfoType::nominal(kind_id("physical/travel-dimension@1"), dimension_type)
            .unwrap();
    let nominal = StructuredInfoValue::nominal(nominal_type, dimension).unwrap();
    let expected = nominal.value_type().profile().unwrap().value_kind().clone();
    assert_eq!(configured(nominal).semantic_kind(), expected);
    assert_ne!(expected, kind_id(DIMENSION_INFO_ID));

    let count_type = StructuredInfoType::leaf(kind_id(COUNT_INFO_ID)).unwrap();
    let count = StructuredInfoValue::leaf(count_type, 1_u64.to_le_bytes().to_vec()).unwrap();
    let expected = count.value_type().profile().unwrap().value_kind().clone();
    assert_eq!(configured(count).semantic_kind(), expected);
    assert_ne!(expected, kind_id(DIMENSION_INFO_ID));
}
