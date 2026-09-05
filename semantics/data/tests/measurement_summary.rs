use conduit_core::{Quantity, QuantityUnit, TemporalInstant, TemporalScale};
use conduit_data::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn profile() -> MeasurementWindowProfile {
    MeasurementWindowProfile {
        capacity: 4,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(i64::MIN, QuantityUnit::Millivolt),
            maximum: Quantity::new(i64::MAX, QuantityUnit::Millivolt),
        },
        clock_basis: "sensor-clock".into(),
        full_policy: FullWindowPolicy::Reject,
    }
}

fn sample(value: i64, ticks: u64) -> MeasurementSample {
    MeasurementSample {
        value: Quantity::new(value, QuantityUnit::Millivolt),
        observed_at: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "sensor-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        uncertainty: None,
    }
}

#[test]
fn mean_minimum_maximum_and_range_retain_exact_unit_and_window_time() {
    let mut window = BoundedMeasurementWindow::new(profile()).unwrap();
    for (value, ticks) in [(2, 10), (6, 20), (10, 30)] {
        window.push(sample(value, ticks)).unwrap();
    }
    let summary = summarize_measurement_window(&window).unwrap();
    assert_eq!(summary.sample_count, 3);
    assert_eq!(summary.mean, Quantity::new(6, QuantityUnit::Millivolt));
    assert_eq!(summary.minimum, Quantity::new(2, QuantityUnit::Millivolt));
    assert_eq!(summary.maximum, Quantity::new(10, QuantityUnit::Millivolt));
    assert_eq!(summary.range, Quantity::new(8, QuantityUnit::Millivolt));
    assert_eq!(summary.first_observed_at.ticks, 10);
    assert_eq!(summary.last_observed_at.ticks, 30);
}

#[test]
fn empty_inexact_and_overflow_outcomes_are_distinct() {
    let empty = BoundedMeasurementWindow::new(profile()).unwrap();
    assert_eq!(
        summarize_measurement_window(&empty),
        Err(MeasurementSummaryRefusal::EmptyWindow)
    );

    let mut inexact = BoundedMeasurementWindow::new(profile()).unwrap();
    inexact.push(sample(1, 1)).unwrap();
    inexact.push(sample(2, 2)).unwrap();
    assert_eq!(
        summarize_measurement_window(&inexact),
        Err(MeasurementSummaryRefusal::InexactMean)
    );

    let mut overflow = BoundedMeasurementWindow::new(profile()).unwrap();
    overflow.push(sample(i64::MIN, 1)).unwrap();
    overflow.push(sample(0, 2)).unwrap();
    assert_eq!(
        summarize_measurement_window(&overflow),
        Err(MeasurementSummaryRefusal::ArithmeticOverflow)
    );
}

#[test]
fn canonical_summary_is_a_reusable_exact_typed_form() {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut catalog).unwrap();
    let source = include_str!("../../../forms/measurement-summary/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "measurement-summary", &catalog).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let gear = &authored.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), MEASUREMENT_SUMMARY_KIND);
    assert_eq!(
        gear.kind_contract_revision.as_str(),
        MEASUREMENT_SUMMARY_CONTRACT_REVISION
    );
}
