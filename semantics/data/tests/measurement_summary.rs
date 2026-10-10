use conduit_core::{Quantity, TemporalInstant, TemporalScale, Unit};
use conduit_data::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn profile() -> MeasurementWindowProfile {
    MeasurementWindowProfile {
        capacity: 4,
        range: MeasurementRange {
            minimum: Quantity::new(i64::MIN, Unit::Millivolt),
            maximum: Quantity::new(i64::MAX, Unit::Millivolt),
        },
        clock_basis: "sensor-clock".into(),
        full_policy: FullWindowPolicy::Reject,
    }
}

fn sample(value: i64, ticks: u64) -> MeasurementSample {
    MeasurementSample {
        value: Quantity::new(value, Unit::Millivolt),
        observed_at: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "sensor-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        }
        .try_into()
        .unwrap(),
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
    assert_eq!(summary.mean, Quantity::new(6, Unit::Millivolt));
    assert_eq!(summary.minimum, Quantity::new(2, Unit::Millivolt));
    assert_eq!(summary.maximum, Quantity::new(10, Unit::Millivolt));
    assert_eq!(summary.range, Quantity::new(8, Unit::Millivolt));
    assert_eq!(summary.first_observed_at.ticks, 10);
    assert_eq!(summary.last_observed_at.ticks, 30);
}

#[test]
fn empty_nonterminating_and_wide_range_outcomes_are_distinct() {
    let empty = BoundedMeasurementWindow::new(profile()).unwrap();
    assert_eq!(
        summarize_measurement_window(&empty),
        Err(MeasurementSummaryRefusal::EmptyWindow)
    );

    let mut inexact = BoundedMeasurementWindow::new(profile()).unwrap();
    inexact.push(sample(1, 1)).unwrap();
    inexact.push(sample(2, 2)).unwrap();
    assert_eq!(
        summarize_measurement_window(&inexact).unwrap().mean,
        Quantity::from_decimal(15, -1, Unit::Millivolt).unwrap()
    );
    inexact.push(sample(2, 3)).unwrap();
    assert_eq!(
        summarize_measurement_window(&inexact),
        Err(MeasurementSummaryRefusal::InexactMean)
    );

    let mut overflow = BoundedMeasurementWindow::new(profile()).unwrap();
    overflow.push(sample(i64::MIN, 1)).unwrap();
    overflow.push(sample(0, 2)).unwrap();
    assert_eq!(
        summarize_measurement_window(&overflow).unwrap().range,
        Quantity::from_decimal(9_223_372_036_854_775_808, 0, Unit::Millivolt).unwrap()
    );
}

#[test]
fn canonical_summary_is_a_reusable_exact_typed_plot() {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_threshold_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut catalog).unwrap();
    conduit_little_seismograph_fixture::install_little_seismograph_fixture_catalog(
        &mut startup,
        &mut catalog,
    )
    .unwrap();
    let source = include_str!("../../../plots/little-seismograph/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "measurement-summary", &catalog).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let gear = &authored.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), MEASUREMENT_SUMMARY_KIND);
    assert_eq!(
        gear.kind_contract_revision.as_str(),
        MEASUREMENT_SUMMARY_CONTRACT_REVISION
    );
}

#[test]
fn fractional_measurements_and_decimal_mean_survive_the_canonical_wire() {
    let mut window = BoundedMeasurementWindow::new(profile()).unwrap();
    let mut first = sample(0, 1);
    first.value = Quantity::parse_plot_literal("0.15mV").unwrap();
    let mut second = sample(0, 2);
    second.value = Quantity::parse_plot_literal("0.2mV").unwrap();
    window.push(first).unwrap();
    window.push(second).unwrap();
    let summary = summarize_measurement_window(&window).unwrap();
    assert_eq!(
        summary.mean,
        Quantity::parse_plot_literal("0.175mV").unwrap()
    );
    let bytes = encode_measurement_summary(&summary).unwrap();
    assert_eq!(decode_measurement_summary(&bytes).unwrap(), summary);
}

#[test]
fn temperature_points_are_retained_without_inventing_point_ranges_or_uncertainty() {
    let mut temperature_profile = profile();
    temperature_profile.range = MeasurementRange {
        minimum: Quantity::parse_plot_literal("0°C").unwrap(),
        maximum: Quantity::parse_plot_literal("100°C").unwrap(),
    };
    let mut window = BoundedMeasurementWindow::new(temperature_profile).unwrap();
    let mut point = sample(0, 1);
    point.value = Quantity::parse_plot_literal("21°C").unwrap();
    assert_eq!(
        decode_measurement_sample(&encode_measurement_sample(&point).unwrap()).unwrap(),
        point
    );
    window.push(point.clone()).unwrap();
    assert_eq!(
        summarize_measurement_window(&window),
        Err(MeasurementSummaryRefusal::PointDifferenceRequired)
    );
    point.observed_at = sample(0, 2).observed_at;
    point.uncertainty = Some(Quantity::parse_plot_literal("2°C").unwrap());
    assert_eq!(
        window.push(point.clone()),
        Err(MeasurementWindowRefusal::PointDifferenceRequired)
    );
    assert_eq!(
        encode_measurement_sample(&point),
        Err(MeasurementWireRefusal::Malformed)
    );
}

#[test]
fn zero_measurements_do_not_force_materializing_high_decimal_powers() {
    let large = Quantity::from_decimal(1, 100, Unit::Millivolt).unwrap();
    let mut large_profile = profile();
    large_profile.range.maximum = large;
    large_profile.range.minimum = Quantity::new(0, Unit::Millivolt);
    let mut window = BoundedMeasurementWindow::new(large_profile).unwrap();
    let zero = sample(0, 1);
    let mut point = sample(0, 2);
    point.value = large;
    window.push(zero).unwrap();
    window.push(point).unwrap();
    let summary = summarize_measurement_window(&window).unwrap();
    assert_eq!(
        summary.mean,
        Quantity::from_decimal(5, 99, Unit::Millivolt).unwrap()
    );
    assert_eq!(summary.range, large);
    assert_eq!(
        decode_measurement_summary(&encode_measurement_summary(&summary).unwrap()).unwrap(),
        summary
    );
}

#[test]
fn paired_delta_measurements_preserve_role_through_summary_and_wire() {
    let delta = |value| {
        Quantity::from_decimal_role(value, 0, Unit::Celsius, conduit_core::QuantityRole::Delta)
            .unwrap()
    };
    let mut delta_profile = profile();
    delta_profile.range = MeasurementRange {
        minimum: delta(-100),
        maximum: delta(100),
    };
    let mut window = BoundedMeasurementWindow::new(delta_profile).unwrap();
    for (value, ticks) in [(2, 1), (4, 2)] {
        let mut measurement = sample(0, ticks);
        measurement.value = delta(value);
        measurement.uncertainty = Some(delta(1));
        window.push(measurement).unwrap();
    }
    let summary = summarize_measurement_window(&window).unwrap();
    assert_eq!(summary.mean, delta(3));
    assert_eq!(summary.range, delta(2));
    assert_eq!(
        decode_measurement_summary(&encode_measurement_summary(&summary).unwrap()).unwrap(),
        summary
    );
    let mut mixed = sample(0, 3);
    mixed.value = Quantity::new(3, Unit::Celsius);
    assert_eq!(
        window.push(mixed),
        Err(MeasurementWindowRefusal::UnitMismatch)
    );
}
