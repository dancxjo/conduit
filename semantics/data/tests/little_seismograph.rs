use conduit_core::{Quantity, QuantityUnit, TemporalInstant, TemporalScale};
use conduit_data::{
    install_measurement_plot_catalog, install_measurement_summary_catalog,
    install_measurement_threshold_catalog, install_measurement_window_catalog,
    summarize_measurement_window, BoundedMeasurementWindow, FullWindowPolicy,
    MeasurementHysteresis, MeasurementPlotOverflowPolicy, MeasurementPlotProfile,
    MeasurementPlotSeries, MeasurementRange, MeasurementSample, MeasurementThresholdPolicy,
    MeasurementThresholdState, MeasurementThresholdTransition, MeasurementWindowProfile,
    MEASUREMENT_COUNT_WINDOW_KIND, MEASUREMENT_HYSTERESIS_KIND, MEASUREMENT_PLOT_KIND,
    MEASUREMENT_SUMMARY_KIND, PLOT_AXIS_MILLIONTHS,
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

fn sample(value: i64, ticks: u64) -> MeasurementSample {
    MeasurementSample {
        value: Quantity::new(value, QuantityUnit::Millivolt),
        observed_at: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "deterministic-source-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        uncertainty: Some(Quantity::new(1, QuantityUnit::Millivolt)),
    }
}

#[test]
fn deterministic_source_runs_the_exact_processing_pipeline() {
    let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
        capacity: 4,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(-100, QuantityUnit::Millivolt),
            maximum: Quantity::new(100, QuantityUnit::Millivolt),
        },
        clock_basis: "deterministic-source-clock".into(),
        full_policy: FullWindowPolicy::DropOldest,
    })
    .unwrap();
    for (value, ticks) in [(-100, 1), (-50, 2), (0, 3), (50, 4), (100, 5)] {
        window.push(sample(value, ticks)).unwrap();
    }
    assert_eq!(window.discarded_samples(), 1);
    assert_eq!(
        window
            .samples()
            .iter()
            .map(|entry| entry.value.value())
            .collect::<Vec<_>>(),
        [-50, 0, 50, 100]
    );

    let summary = summarize_measurement_window(&window).unwrap();
    assert_eq!(summary.mean, Quantity::new(25, QuantityUnit::Millivolt));
    assert_eq!(summary.minimum, Quantity::new(-50, QuantityUnit::Millivolt));
    assert_eq!(summary.maximum, Quantity::new(100, QuantityUnit::Millivolt));
    assert_eq!(summary.range, Quantity::new(150, QuantityUnit::Millivolt));

    let mut threshold = MeasurementHysteresis::new(
        MeasurementThresholdPolicy {
            lower: Quantity::new(10, QuantityUnit::Millivolt),
            upper: Quantity::new(20, QuantityUnit::Millivolt),
        },
        MeasurementThresholdState::Below,
    )
    .unwrap();
    let decision = threshold.evaluate(&summary).unwrap();
    assert_eq!(decision.state, MeasurementThresholdState::Above);
    assert_eq!(
        decision.transition,
        Some(MeasurementThresholdTransition::RoseAbove)
    );

    let plot = MeasurementPlotSeries::project(
        &window,
        MeasurementPlotProfile {
            point_capacity: 3,
            overflow_policy: MeasurementPlotOverflowPolicy::EvenlySpaced,
        },
    )
    .unwrap();
    assert_eq!(plot.source_samples(), 4);
    assert_eq!(plot.omitted_samples(), 1);
    assert_eq!(
        plot.points()
            .iter()
            .map(|point| (
                point.source_index,
                point.time_millionths,
                point.value_millionths
            ))
            .collect::<Vec<_>>(),
        [
            (0, 0, 250_000),
            (1, 333_333, 500_000),
            (3, PLOT_AXIS_MILLIONTHS, PLOT_AXIS_MILLIONTHS),
        ]
    );
}
