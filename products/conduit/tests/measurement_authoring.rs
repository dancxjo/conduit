//! Actual public bounded compiler context plus isolated requested-backing probe.
#[path = "../../../targets/std/src/allocation_probe.rs"]
mod allocation_probe;
// Import the product source boundary; this diagnostic does not use file loading
// or its separate non-authoring expansion entrance.
#[allow(dead_code)]
#[path = "../src/plot_source.rs"]
mod plot_source;
use conduit_plot::{ProfileCatalog, StartupCatalog};

#[test]
fn ordinary_and_bounded_product_contexts_check_the_same_three_series_source() {
    let text = plot_source::measurement_chart_fixture_source();
    let canonical = plot_source::parse(&text).unwrap();
    let expanded = canonical.expand_entry_for_authoring().unwrap();
    let context = conduit::source_check_worker::standard_context().unwrap();
    let mut projection = conduit_plot::SourceCheckProjection::new(
        "diagnostic/measurement-authoring",
        1,
        &text,
        context,
    )
    .unwrap();
    let info = projection
        .project(&conduit_plot::SourceCheckRequest {
            snapshot: projection.snapshot().clone(),
            context: projection.context().clone(),
            entry: "inspection/three-series".into(),
            limits: conduit_plot::SourceCheckLimits::default(),
        })
        .unwrap();
    assert_eq!(
        info.check,
        conduit_plot::CheckOutcome::Accepted,
        "{:?}",
        info.diagnostics
    );
    assert_eq!(
        info.candidate.checked_plot_id.as_deref(),
        Some(expanded.expanded.checked_plot_id.as_str())
    );
    assert_eq!(info.plan, conduit_plot::RealizationPresence::Absent);
    assert_eq!(info.play, conduit_plot::RealizationPresence::Absent);
}

#[test]
fn measurement_authoring_metadata_has_explicit_requested_backing_cost() {
    // Existing operator-selected caller envelope, not a larger resource grant.
    // Current-thread requested Rust bytes exclude process/C/stack/other threads;
    // this CPU diagnostic is not whole-Root or physical admission qualification.
    const CALLER_CEILING: usize = 128 * 1024 * 1024;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_data::install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    let probe = crate::allocation_probe::begin();
    plot_source::install_measurement_catalog_for_cost_observation(&mut startup, &mut profiles)
        .unwrap();
    let extra = probe.finish_statistics();
    assert!(extra.live_bytes > 0);
    assert!(extra.peak_live_bytes <= CALLER_CEILING);
    println!(
        "Finite chart authoring metadata extra retained/preparation requested backing: {extra:?}"
    );
    let probe = crate::allocation_probe::begin();
    let (startup, profiles) = plot_source::fresh_catalogs_for_cost_observation().unwrap();
    let context = conduit_plot::SourceCheckContext::new(
        conduit_plot::CheckContextRevision {
            grammar_edition: "conduitese-v1".into(),
            catalog: "diagnostic/product-standard".into(),
            kinds_fores: "diagnostic/product-standard".into(),
            types_laws: "diagnostic/product-standard".into(),
            backs: "empty@1".into(),
        },
        startup,
        profiles,
        conduit_plot::CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut projection = conduit_plot::SourceCheckProjection::new(
        "diagnostic/chart-cost",
        1,
        &plot_source::measurement_chart_fixture_source(),
        context,
    )
    .unwrap();
    let info = projection
        .project(&conduit_plot::SourceCheckRequest {
            snapshot: projection.snapshot().clone(),
            context: projection.context().clone(),
            entry: "inspection/three-series".into(),
            limits: conduit_plot::SourceCheckLimits::default(),
        })
        .unwrap();
    assert_eq!(
        info.check,
        conduit_plot::CheckOutcome::Accepted,
        "{:?}",
        info.diagnostics
    );
    let total = probe.finish_statistics();
    println!("Fresh full standard catalog plus exact chart checking requested backing: {total:?}; unchanged caller ceiling={CALLER_CEILING}");
    assert!(
        total.peak_live_bytes <= CALLER_CEILING,
        "refuse rather than silently enlarge original caller envelope"
    );
    std::hint::black_box((&projection, &info));
}
