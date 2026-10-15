use conduit_core::*;
#[test]
fn explicit_representation_view_preserves_nominal_type_and_strict_admission() {
    let leaf = StructuredInfoType::leaf(kind_id(F64_INFO_ID)).unwrap();
    let power =
        StructuredInfoType::nominal(kind_id("test/full-scale-power"), leaf.clone()).unwrap();
    let observed =
        StructuredInfoType::nominal(kind_id("test/observed-power"), power.clone()).unwrap();
    let bytes = IeeeF64::from_value(0.25).encode();
    let value = StructuredInfoValue::leaf(leaf.clone(), bytes.to_vec()).unwrap();
    let value = StructuredInfoValue::nominal(power, value).unwrap();
    let encoded = StructuredInfoValue::nominal(observed.clone(), value)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let view = validate_canonical_structured_value(&encoded).unwrap();
    assert_eq!(
        view.primitive_bytes(F64_INFO_ID),
        Err(StructuredInfoRefusal::WrongType)
    );
    assert_eq!(
        view.primitive_representation_bytes(F64_INFO_ID).unwrap(),
        bytes
    );
    assert_eq!(view.type_bytes(), observed.canonical_bytes().unwrap());
    assert_eq!(
        view.primitive_representation_bytes("value/u64"),
        Err(StructuredInfoRefusal::WrongType)
    );
    let plain = StructuredInfoValue::leaf(leaf, bytes.to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        validate_canonical_structured_value(&plain)
            .unwrap()
            .primitive_representation_bytes(F64_INFO_ID)
            .unwrap(),
        bytes
    );
}
