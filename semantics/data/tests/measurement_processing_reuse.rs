use conduit_data::{
    install_measurement_plot_catalog, install_measurement_plot_form_back,
    install_measurement_plot_form_catalog, install_measurement_summary_catalog,
    install_measurement_threshold_catalog, install_measurement_window_catalog,
    MEASUREMENT_PLOT_FORM_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring_with_backs, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_threshold_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profile).unwrap();
    conduit_data::install_little_seismograph_fixture_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_plot_form_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn bench_consumes_the_exact_little_seismograph_plot_back_without_copying_source() {
    let (startup, profile) = catalogs();
    let bench_source = include_str!("../../../forms/bench-telemetry/main.conduit");
    let bench = check_syntax_document(&parse_syntax_document(bench_source), &startup).unwrap();
    let processing_source = include_str!("../../../forms/little-seismograph/main.conduit");
    let processing =
        check_syntax_document(&parse_syntax_document(processing_source), &startup).unwrap();
    let plot_form = processing
        .forms
        .iter()
        .find(|form| form.name == "measurement-plot")
        .unwrap();

    let mut backs = CanonicalBackCatalog::new();
    install_measurement_plot_form_back(&startup, &profile, &mut backs).unwrap();
    let expanded =
        expand_canonical_form_for_authoring_with_backs(&bench, "bench-telemetry", &profile, &backs)
            .unwrap()
            .expanded;

    assert_eq!(expanded.realization_backs.len(), 1);
    let selected = &expanded.realization_backs[0];
    assert_eq!(selected.kind_id.as_str(), MEASUREMENT_PLOT_FORM_KIND);
    assert_eq!(selected.invocation_path, "bench-telemetry/plot");
    assert_eq!(selected.source_document_id, processing.source_document_id);
    assert_eq!(selected.checked_form_id, plot_form.checked_form_id);
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.gear_id.as_str() == "bench-telemetry/plot/project"));
    assert_ne!(bench.source_document_id, processing.source_document_id);
    expanded.validate_expansion().unwrap();
}
