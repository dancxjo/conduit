use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_core::ConfigurationValue;

fn check(source: &str) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    check_syntax_document(&syntax, &startup).unwrap();
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn catalogs_with_flow_state() -> (StartupCatalog, ProfileCatalog) {
    let (mut startup, mut profile) = catalogs();
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
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

#[test]
fn quantity_range_map_lowers_with_production_map_quantity_contract_and_exact_units() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../forms/quantity-range-map/main.conduit");
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =\n        expand_canonical_form_for_authoring(&checked, "quantity-range-map", &profile).unwrap();
    let map = authored
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "math/map-quantity")
        .unwrap();
    assert_eq!(
        map.configuration
            .iter()
            .find(|entry| entry.key == "unit")
            .unwrap()
            .value,
        ConfigurationValue::Text("Hz".into())
    );
    assert_eq!(
        map.configuration
            .iter()
            .find(|entry| entry.key == "target-minimum")
            .unwrap()
            .value,
        ConfigurationValue::I64(20)
    );
    assert_eq!(
        map.configuration
            .iter()
            .find(|entry| entry.key == "target-maximum")
            .unwrap()
            .value,
        ConfigurationValue::I64(20_000)
    );
}

#[test]
fn production_quantity_map_refuses_invalid_enum_fields() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../forms/quantity-range-map/main.conduit");
    let wrong_unit = source.replace("unit = \"Hz\"", "unit = \"GHz\"");
    let parsed = parse_syntax_document(&wrong_unit);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let error =\n        expand_canonical_form_for_authoring(&checked, "quantity-range-map", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("unit"));
}

#[test]
fn target_maximum_value_is_not_semantically_bounded_by_current_quantity_map_contract() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../forms/quantity-range-map/main.conduit")
        .replace("target-maximum = 20000", "target-maximum = 200000000000");
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =\n        expand_canonical_form_for_authoring(&checked, "quantity-range-map", &profile).unwrap();
    let map = authored
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "math/map-quantity")
        .unwrap();
    assert_eq!(
        map.configuration
            .iter()
            .find(|entry| entry.key == "target-maximum")
            .unwrap()
            .value,
        ConfigurationValue::I64(200_000_000_000)
    );
}

#[test]
#[ignore = "Enable after quantity-map enforces semantic target-maximum bounds"]
fn target_maximum_should_refuse_after_quantity_map_range_enforcement() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../forms/quantity-range-map/main.conduit")
        .replace("target-maximum = 20000", "target-maximum = 200000000000");
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let error =\n        expand_canonical_form_for_authoring(&checked, "quantity-range-map", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("target-maximum"));
}

#[test]
fn canonical_distance_frequency_surface_holes_are_reported_without_private_catalog_fixtures() {
    let map_range_source = "form pocket-theremin-distance-frequency (\n    distance: Quantity > frequency: $Quantity\n) {\n    map: map/range(source-minimum = 0cm, source-maximum = 30cm, target-minimum = 220Hz, target-maximum = 880Hz)\n    distance > map.in\n    map.out > frequency\n    .\n}\n";
    let parsed = parse_syntax_document(map_range_source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (startup, _) = catalogs();
    let error = check_syntax_document(&parsed, &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-028");
    assert!(error.message.contains("map/range"));

    let keep_surface = "form keep-frequency (\n    distance: Quantity > frequency: $Quantity\n) {\n    map: math/map-quantity(source-minimum = 0, source-maximum = 1000000, target-minimum = 220, target-maximum = 880, target-granularity = 1, unit = \"Hz\", range-policy = \"clamp\", quantization = \"nearest\")\n    keep Frequency(440Hz) for this play\n    distance > map.in\n    map.out > frequency\n    .\n}\n";
    let parsed = parse_syntax_document(keep_surface);
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].code, "CND-FRM-019");
    assert!(parsed.diagnostics[0]\n        .message\n        .contains("keep Frequency(440Hz) for this play"));
}

#[test]
fn production_catalog_out_of_range_validation_still_runs_in_normal_tests() {
    let (startup, profile) = catalogs_with_flow_state();
    let source = "form gate-bounds {\n    gate: flow/gate(maximum-enable-updates = 0)\n}\n";
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let error = expand_canonical_form_for_authoring(&checked, "gate-bounds", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-040");
    assert!(error.message.contains("maximum-enable-updates"));
}
