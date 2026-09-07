use conduit_data::{
    install_measurement_plot_catalog, install_measurement_summary_catalog,
    install_measurement_threshold_catalog, install_measurement_window_catalog,
    MEASUREMENT_COUNT_WINDOW_KIND, MEASUREMENT_HYSTERESIS_KIND, MEASUREMENT_PLOT_KIND,
    MEASUREMENT_SUMMARY_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/little-seismograph/main.conduit");

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_threshold_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn processing_composes_four_reusable_forms_without_source_copying() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    assert_eq!(checked.forms.len(), 5);
    let authored =
        expand_canonical_form_for_authoring(&checked, "little-seismograph-processing", &profile)
            .unwrap();
    let kinds = authored
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    for expected in [
        MEASUREMENT_COUNT_WINDOW_KIND,
        MEASUREMENT_SUMMARY_KIND,
        MEASUREMENT_HYSTERESIS_KIND,
        MEASUREMENT_PLOT_KIND,
    ] {
        assert!(kinds.contains(&expected), "missing primitive {expected}");
    }
    assert_eq!(authored.expanded.gears.len(), 4);
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 4);
}

#[test]
fn canonical_processing_meaning_is_host_and_mechanism_neutral() {
    let source = SOURCE.to_ascii_lowercase();
    for forbidden in [
        "browser",
        "dom",
        "canvas",
        "webserial",
        "webusb",
        "device",
        "hostid",
        "bootid",
        "sensor",
        "pin",
        "indexeddb",
    ] {
        assert!(
            !source.contains(forbidden),
            "authored source leaked {forbidden}"
        );
    }
}
