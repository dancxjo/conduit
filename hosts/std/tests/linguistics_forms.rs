use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PortTemporal, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use conduit_language::{
    annotate_with_model_fixture, annotate_with_unicode_library, install_linguistics_catalogs,
    linguistic_token_type, linguistic_tokens_four_type, tokenize_four, LinguisticRefusal,
    ANNOTATE_FOUR_KIND, LINGUISTIC_DEPENDENCY_COUNT, LINGUISTIC_FEATURE_SLOTS,
    LINGUISTIC_TOKEN_COUNT, MAXIMUM_LINGUISTIC_TEXT_BYTES, TOKENIZE_FOUR_KIND,
};
use conduit_std_host::hosted_linguistics::{linguistics_std_offers, LINGUISTICS_HOST_OPERATION};

const SOURCE: &str = include_str!("../../../examples/linguistic-annotations.conduit");

#[test]
fn canonical_form_tokenizes_and_projects_annotations_without_json() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let mut selector_offers = Vec::new();
    for stage in checked.forms[0]
        .cords
        .iter()
        .flat_map(|cord| cord.stages.iter())
    {
        if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
            profile
                .insert(structured_selector_definition(
                    selector,
                    PortTemporal::Value,
                ))
                .unwrap();
            selector_offers.push(conduit_std_catalog::structured_selector_std_offer(
                selector,
                PortTemporal::Value,
            ));
        }
    }
    assert_eq!(selector_offers.len(), 3);
    let authored =
        expand_canonical_form_for_authoring(&checked, "linguistic-annotations", &profile).unwrap();
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == TOKENIZE_FOUR_KIND));
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == ANNOTATE_FOUR_KIND));
    assert_eq!(authored.output_bindings.len(), 1);

    let mut offers = linguistics_std_offers();
    offers.extend(selector_offers);
    let host = host(offers);
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let annotation = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == ANNOTATE_FOUR_KIND)
        .unwrap();
    assert_eq!(
        annotation.host_operations[0].contract_id.as_str(),
        LINGUISTICS_HOST_OPERATION
    );
}

#[test]
fn tokenizer_uses_unicode_scalar_spans_and_explicit_optional_fields() {
    let tokens = tokenize_four("text/example", "Élan stars shine.").unwrap();
    assert_eq!(
        provenance_tag(record_field(&tokens, "provenance")),
        "deterministic_rule"
    );
    let tokens = collection_field(&tokens, "tokens");
    assert_eq!(tokens.len(), usize::from(LINGUISTIC_TOKEN_COUNT));
    assert_eq!(leaf_text(record_field(&tokens[0], "surface")), "Élan");
    assert_eq!(span_bounds(record_field(&tokens[0], "span")), (0, 4));
    assert_eq!(span_bounds(record_field(&tokens[1], "span")), (5, 10));
    assert_eq!(variant_tag(record_field(&tokens[0], "lemma")), "absent");
    let features = collection_field(&tokens[0], "features");
    assert_eq!(features.len(), usize::from(LINGUISTIC_FEATURE_SLOTS));
    assert!(features
        .iter()
        .all(|feature| variant_tag(feature) == "unused"));

    let token_type = linguistic_token_type();
    let StructuredInfoTypeShape::Record { fields, .. } = token_type.shape() else {
        panic!("token must remain structured Info")
    };
    assert!(fields.iter().any(|field| field.name() == "identity"));
    assert!(fields.iter().any(|field| field.name() == "span"));
}

#[test]
fn library_annotations_and_dependencies_are_finite_and_exact() {
    let tokens = tokenize_four("text/example", "Bright stars shine.").unwrap();
    let annotated = annotate_with_unicode_library(&tokens).unwrap();
    assert_eq!(
        provenance_tag(record_field(&annotated, "provenance")),
        "library"
    );
    assert_eq!(
        collection_field(&annotated, "annotations").len(),
        usize::from(LINGUISTIC_TOKEN_COUNT)
    );
    let dependencies = collection_field(&annotated, "dependencies");
    assert_eq!(dependencies.len(), usize::from(LINGUISTIC_DEPENDENCY_COUNT));
    assert_eq!(
        token_ordinal(record_field(&dependencies[0], "dependent")),
        0
    );
    assert_eq!(token_ordinal(record_field(&dependencies[0], "governor")), 1);
    assert_eq!(
        leaf_text(record_field(
            record_field(&dependencies[0], "dependent"),
            "text_identity"
        )),
        "text/example"
    );
}

#[test]
fn model_provenance_is_distinct_and_provider_token_ids_are_not_semantics() {
    let tokens = tokenize_four("text/example", "Bright stars shine.").unwrap();
    let modeled = annotate_with_model_fixture(&tokens, "model/example@1").unwrap();
    assert_eq!(
        provenance_tag(record_field(&modeled, "provenance")),
        "model"
    );
    let debug = format!("{modeled:?}");
    assert!(!debug.contains("provider-token-91"));
    let round_trip =
        StructuredInfoValue::from_canonical_bytes(&modeled.canonical_bytes().unwrap()).unwrap();
    assert_eq!(round_trip, modeled);
}

#[test]
fn tokenizer_refuses_non_four_token_and_oversized_inputs() {
    assert_eq!(
        tokenize_four("text/short", "only three words"),
        Err(LinguisticRefusal::WrongTokenCount {
            expected: LINGUISTIC_TOKEN_COUNT,
            actual: 3,
        })
    );
    let oversized = "a".repeat(MAXIMUM_LINGUISTIC_TEXT_BYTES as usize + 1);
    assert_eq!(
        tokenize_four("text/large", &oversized),
        Err(LinguisticRefusal::TextTooLarge)
    );
    assert!(linguistic_tokens_four_type().profile().is_ok());
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/linguistics-proof"),
        boot_id: BootId::from("boot/linguistics-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/linguistics-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a [StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = record_field(value, name).shape() else {
        panic!("expected collection")
    };
    values
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn provenance_tag(value: &StructuredInfoValue) -> &str {
    variant_tag(value)
}

fn span_bounds(value: &StructuredInfoValue) -> (u64, u64) {
    (
        leaf_text(record_field(value, "start")).parse().unwrap(),
        leaf_text(record_field(value, "end")).parse().unwrap(),
    )
}

fn token_ordinal(value: &StructuredInfoValue) -> u64 {
    leaf_text(record_field(value, "ordinal")).parse().unwrap()
}
