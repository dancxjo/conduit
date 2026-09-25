use conduit_core::{
    ConfigurationValue, PortTemporal, Quantity, QuantityDimension, QuantityUnit, DISTANCE_INFO_ID,
    FREQUENCY_INFO_ID,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn check(source: &str) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &startup).unwrap();
}

#[test]
fn reusable_mapping_and_outside_consumer_are_checked_and_host_neutral() {
    let mapping = include_str!("../../../forms/quantity-range-map/main.conduit");
    let light = include_str!("../../../forms/normalized-light-intensity/main.conduit");
    check(mapping);
    check(light);
    for forbidden in [
        "pointerevent",
        "touchevent",
        "canvas",
        "web audio",
        "dom",
        "pixel",
        "browser",
    ] {
        assert!(!mapping.to_ascii_lowercase().contains(forbidden));
        assert!(!light.to_ascii_lowercase().contains(forbidden));
    }
}

fn theremin_catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn expand_theremin(
    source: &str,
) -> Result<conduit_form::ExpandedAuthoringForm, conduit_form::CanonicalExpansionDiagnostic> {
    let (startup, profile) = theremin_catalogs();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    expand_canonical_form_for_authoring(&checked, "pocket-theremin-distance-frequency", &profile)
}

#[test]
fn canonical_distance_frequency_keep_is_real_production_language() {
    let source =
        include_str!("../../../proof/fixtures/forms/pocket-theremin-distance-frequency.conduit");
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), source);
    for exact in ["0cm", "30cm", "220Hz", "880Hz", "440Hz", ">>", "}."] {
        assert!(source.contains(exact), "missing canonical spelling {exact}");
    }

    let authored = expand_theremin(source).unwrap();
    let expanded = authored.expanded;
    assert_eq!(expanded.gears.len(), 2);

    let map = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::DISTANCE_FREQUENCY_MAP_KIND)
        .unwrap();
    assert_eq!(map.inputs[0].value_kind.as_str(), DISTANCE_INFO_ID);
    assert_eq!(map.outputs[0].value_kind.as_str(), FREQUENCY_INFO_ID);
    assert_eq!(map.inputs[0].temporal, PortTemporal::Value);
    assert_eq!(map.outputs[0].temporal, PortTemporal::Value);

    let retained = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::LATEST_KIND)
        .unwrap();
    assert_eq!(retained.inputs[0].value_kind.as_str(), FREQUENCY_INFO_ID);
    assert_eq!(retained.outputs[0].value_kind.as_str(), FREQUENCY_INFO_ID);
    assert_eq!(retained.outputs[0].temporal, PortTemporal::Current);
    assert_eq!(
        retained
            .configuration
            .iter()
            .find(|entry| entry.key == "retained-duration")
            .map(|entry| &entry.value),
        Some(&ConfigurationValue::Text("play".into()))
    );
    assert_eq!(
        retained
            .configuration
            .iter()
            .find(|entry| entry.key == "initial")
            .map(|entry| &entry.value),
        Some(&ConfigurationValue::Quantity(Quantity::new(
            440,
            QuantityUnit::Hertz
        )))
    );

    assert!(expanded.gears.iter().all(|gear| {
        matches!(
            gear.kind_id.as_str(),
            conduit_semantic_catalog::DISTANCE_FREQUENCY_MAP_KIND
                | conduit_semantic_catalog::LATEST_KIND
        )
    }));
}

#[test]
fn dimension_and_range_mistakes_refuse_on_the_production_path() {
    let source = include_str!("../../../proof/fixtures/forms/pocket-theremin-distance-frequency.conduit");

    let wrong_dimension = source.replacen("source-maximum = 30cm", "source-maximum = 30Hz", 1);
    let error = expand_theremin(&wrong_dimension).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("source-maximum"));

    let outside_range = source.replacen("source-maximum = 30cm", "source-maximum = 10001cm", 1);
    let error = expand_theremin(&outside_range).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("source-maximum"));

    let wrong_keep = source.replacen("Frequency(440Hz)", "Frequency(440cm)", 1);
    let error = expand_theremin(&wrong_keep).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("wrong quantity dimension"));
}

#[test]
fn dimensioned_primitive_ids_share_quantity_bytes_without_erasing_meaning() {
    let distance = Quantity::new(30, QuantityUnit::Centimeter);
    let frequency = Quantity::new(440, QuantityUnit::Hertz);
    assert_eq!(distance.dimension(), QuantityDimension::Length);
    assert_eq!(frequency.dimension(), QuantityDimension::Frequency);
    assert_eq!(
        conduit_core::validate_primitive_info(DISTANCE_INFO_ID, &distance.encode()),
        Ok(())
    );
    assert_eq!(
        conduit_core::validate_primitive_info(FREQUENCY_INFO_ID, &frequency.encode()),
        Ok(())
    );
    assert!(conduit_core::validate_primitive_info(FREQUENCY_INFO_ID, &distance.encode()).is_err());
}
