use conduit_core::StructuredInfoTypeShape;

#[test]
fn purpose_projection_bounds_belong_to_native_types() {
    let purpose = conduit_purpose::purpose_state_type();
    let StructuredInfoTypeShape::Record { fields, .. } = purpose.shape() else {
        panic!("purpose state is a record")
    };
    let obligations = fields
        .iter()
        .find(|field| field.name() == "obligations")
        .unwrap();
    let StructuredInfoTypeShape::Nominal { representation, .. } = obligations.value_type().shape()
    else {
        panic!("purpose obligations retain nominal meaning")
    };
    let StructuredInfoTypeShape::Collection { element, length } = representation.shape() else {
        panic!("purpose obligations are fixed")
    };
    assert_eq!(length, 32);

    let StructuredInfoTypeShape::Record { fields, .. } = element.shape() else {
        panic!("purpose obligation is a record")
    };
    let evidence = fields
        .iter()
        .find(|field| field.name() == "evidence_sign_identities")
        .unwrap();
    let StructuredInfoTypeShape::Nominal { representation, .. } = evidence.value_type().shape()
    else {
        panic!("evidence identities retain nominal meaning")
    };
    let StructuredInfoTypeShape::Collection { length, .. } = representation.shape() else {
        panic!("evidence identities are fixed")
    };
    assert_eq!(length, 8);
}

#[test]
fn catalog_no_longer_authors_the_portable_purpose_schema() {
    let adapter = include_str!("../../catalog/src/purpose_catalog.rs");
    for duplicate in [
        "StructuredInfoType::record",
        "StructuredInfoType::variant",
        "StructuredInfoType::collection",
    ] {
        assert!(!adapter.contains(duplicate), "catalog retained {duplicate}");
    }
}
