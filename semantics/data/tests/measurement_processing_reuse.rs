use conduit_data::{
    install_measurement_plot_catalog, install_measurement_plot_source_back,
    install_measurement_plot_source_catalog, install_measurement_summary_catalog,
    install_measurement_threshold_catalog, install_measurement_window_catalog,
    MEASUREMENT_PLOT_SOURCE_KIND,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring_with_backs, parse_syntax_document,
    CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_threshold_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profile).unwrap();
    conduit_little_seismograph_fixture::install_little_seismograph_fixture_catalog(
        &mut startup,
        &mut profile,
    )
    .unwrap();
    install_measurement_plot_source_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn bench_consumes_the_exact_little_seismograph_plot_back_without_copying_source() {
    let (startup, profile) = catalogs();
    let bench_source = include_str!("../../../plots/bench-telemetry/main.conduit");
    let bench = check_syntax_document(&parse_syntax_document(bench_source), &startup).unwrap();
    let processing_source = include_str!("../../../plots/little-seismograph/main.conduit");
    let processing =
        check_syntax_document(&parse_syntax_document(processing_source), &startup).unwrap();
    let measurement_plot_source = processing
        .plots
        .iter()
        .find(|plot| plot.name == "measurement-plot")
        .unwrap();

    let mut backs = CanonicalBackCatalog::new();
    install_measurement_plot_source_back(&startup, &profile, &mut backs).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring_with_backs(&bench, "bench-telemetry", &profile, &backs)
            .unwrap()
            .expanded;

    assert_eq!(expanded.realization_backs.len(), 1);
    let selected = &expanded.realization_backs[0];
    assert_eq!(selected.kind_id.as_str(), MEASUREMENT_PLOT_SOURCE_KIND);
    assert_eq!(selected.invocation_path, "bench-telemetry/plot");
    assert_eq!(selected.source_document_id, processing.source_document_id);
    assert_eq!(
        selected.checked_plot_id,
        measurement_plot_source.checked_plot_id
    );
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.gear_id.as_str() == "bench-telemetry/plot/project"));
    assert_ne!(bench.source_document_id, processing.source_document_id);
    expanded.validate_expansion().unwrap();
}

#[test]
fn environmental_history_face_reuses_the_same_bounded_plot_back() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../bodies/environment/profiles/history-face.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    install_measurement_plot_source_back(&startup, &profile, &mut backs).unwrap();
    let expanded = expand_canonical_plot_for_authoring_with_backs(
        &checked,
        "environmental-history-face",
        &profile,
        &backs,
    )
    .unwrap()
    .expanded;

    assert_eq!(expanded.realization_backs.len(), 1);
    assert_eq!(
        expanded.realization_backs[0].kind_id.as_str(),
        MEASUREMENT_PLOT_SOURCE_KIND
    );
    assert_eq!(
        expanded.realization_backs[0].invocation_path,
        "environmental-history-face/project"
    );
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.gear_id.as_str() == "environmental-history-face/project/project"));
    expanded.validate_expansion().unwrap();
}
