use conduit_core::{ConfigurationValue, StructuredInfoTypeShape};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../forms/homeostasis.conduit");

#[test]
fn homeostasis_is_one_ordinary_checked_authority_free_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile).unwrap();
    conduit_pete::install_homeostasis_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "pete-homeostasis", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        conduit_pete::HOMEOSTASIS_REDUCE_KIND
    );
    assert_eq!(authored.expanded.gears[0].inputs.len(), 6);
    assert_eq!(authored.expanded.gears[0].outputs.len(), 1);
    assert_eq!(authored.expanded.gears[0].configuration.len(), 7);
    assert!(authored.expanded.gears[0]
        .configuration
        .iter()
        .any(|entry| {
            entry.key == "policy-revision"
                && entry.value
                    == ConfigurationValue::Text("conduit.pete/homeostasis-thresholds@1".into())
        }));
    assert!(authored.expanded.gears[0]
        .configuration
        .iter()
        .any(|entry| {
            entry.key == "energy-critical-permille" && entry.value == ConfigurationValue::U64(100)
        }));
    for forbidden in [
        "host/",
        "provider/",
        "authority",
        "dock",
        "prompt",
        "hungry",
    ] {
        assert!(!SOURCE.contains(forbidden), "form leaked {forbidden}");
    }
}

#[test]
fn observation_inputs_keep_domain_values_typed() {
    let types = conduit_pete::homeostasis_registered_types();
    let power = &types
        .iter()
        .find(|(name, _)| *name == "PetePowerObservation")
        .unwrap()
        .1;
    let thermal = &types
        .iter()
        .find(|(name, _)| *name == "PeteThermalObservation")
        .unwrap()
        .1;
    assert!(contains_leaf(power, "value/bool"));
    assert!(contains_leaf(power, "value/count"));
    assert!(contains_leaf(thermal, "value/scalar"));
    assert!(contains_variant(power, "present"));
    assert!(contains_variant(power, "missing"));
    assert!(contains_variant(power, "unavailable"));
}

fn contains_leaf(value: &conduit_core::StructuredInfoType, identity: &str) -> bool {
    match value.shape() {
        StructuredInfoTypeShape::Leaf(kind) => kind.as_str() == identity,
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => contains_leaf(element, identity),
        StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .any(|field| contains_leaf(field.value_type(), identity)),
        StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .any(|case| contains_leaf(case.payload_type(), identity)),
    }
}

fn contains_variant(value: &conduit_core::StructuredInfoType, tag: &str) -> bool {
    match value.shape() {
        StructuredInfoTypeShape::Leaf(_) => false,
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => contains_variant(element, tag),
        StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .any(|field| contains_variant(field.value_type(), tag)),
        StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .any(|case| case.tag() == tag || contains_variant(case.payload_type(), tag)),
    }
}
