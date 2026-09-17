use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const READINESS: &str = include_str!("../../../forms/fulfillment-readiness/main.conduit");

#[test]
fn readiness_is_a_portable_deterministic_semantic_derivation() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_purpose_catalogs(&mut startup, &mut profile).unwrap();

    let parsed = parse_syntax_document(READINESS);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded =
        expand_canonical_form_for_authoring(&checked, "fulfillment-readiness", &profile).unwrap();
    assert_eq!(
        expanded.expanded.gears[0].kind_id.as_str(),
        "purpose/fulfillment-readiness"
    );
    for forbidden in [
        "host/",
        "provider",
        "model",
        "prompt",
        "body.fulfill",
        "execute",
    ] {
        assert!(
            !READINESS.contains(forbidden),
            "portable source leaked {forbidden}"
        );
    }
}

#[test]
fn purpose_evidence_has_exact_finite_bounds() {
    use conduit_core::StructuredInfoTypeShape;
    let purpose = conduit_semantic_catalog::purpose_state_type();
    let StructuredInfoTypeShape::Record { fields, .. } = purpose.shape() else {
        panic!()
    };
    let obligations = fields
        .iter()
        .find(|field| field.name() == "obligations")
        .unwrap();
    let StructuredInfoTypeShape::Collection { element, length } = obligations.value_type().shape()
    else {
        panic!()
    };
    assert_eq!(length, 32);
    let StructuredInfoTypeShape::Record { fields, .. } = element.shape() else {
        panic!()
    };
    let evidence = fields
        .iter()
        .find(|field| field.name() == "evidence_sign_identities")
        .unwrap();
    let StructuredInfoTypeShape::Collection { length, .. } = evidence.value_type().shape() else {
        panic!()
    };
    assert_eq!(length, 8);
}
