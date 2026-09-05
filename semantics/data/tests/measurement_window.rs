use conduit_core::{Quantity, QuantityUnit, TemporalInstant, TemporalScale};
use conduit_data::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn profile(policy: FullWindowPolicy) -> MeasurementWindowProfile {
    MeasurementWindowProfile {
        capacity: 3,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(-2_000, QuantityUnit::Millivolt),
            maximum: Quantity::new(2_000, QuantityUnit::Millivolt),
        },
        clock_basis: "sensor-clock".into(),
        full_policy: policy,
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
        uncertainty: Some(Quantity::new(2, QuantityUnit::Millivolt)),
    }
}

#[test]
fn count_window_is_finite_and_pressure_policy_is_explicit() {
    let mut reject = BoundedMeasurementWindow::new(profile(FullWindowPolicy::Reject)).unwrap();
    for index in 0..3 {
        reject.push(sample(index, index as u64 + 1)).unwrap();
    }
    assert_eq!(
        reject.push(sample(3, 4)),
        Err(MeasurementWindowRefusal::Full)
    );
    assert_eq!(
        reject
            .samples()
            .iter()
            .map(|value| value.value.value())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );

    let mut sliding = BoundedMeasurementWindow::new(profile(FullWindowPolicy::DropOldest)).unwrap();
    for index in 0..4 {
        sliding.push(sample(index, index as u64 + 1)).unwrap();
    }
    assert_eq!(
        sliding
            .samples()
            .iter()
            .map(|value| value.value.value())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(sliding.discarded_samples(), 1);
}

#[test]
fn unit_range_clock_and_timestamp_refusals_stay_distinct() {
    let mut window = BoundedMeasurementWindow::new(profile(FullWindowPolicy::Reject)).unwrap();
    window.push(sample(100, 2)).unwrap();

    let mut wrong_unit = sample(100, 3);
    wrong_unit.value = Quantity::new(100, QuantityUnit::Millimeter);
    assert_eq!(
        window.push(wrong_unit),
        Err(MeasurementWindowRefusal::UnitMismatch)
    );

    assert_eq!(
        window.push(sample(2_001, 3)),
        Err(MeasurementWindowRefusal::OutOfRange)
    );

    let mut wrong_clock = sample(100, 3);
    wrong_clock.observed_at.clock_basis = "other-clock".into();
    assert_eq!(
        window.push(wrong_clock),
        Err(MeasurementWindowRefusal::ClockMismatch)
    );

    assert_eq!(
        window.push(sample(100, 1)),
        Err(MeasurementWindowRefusal::TimestampRegression)
    );

    let mut invalid_timestamp = sample(100, u64::MAX);
    invalid_timestamp.observed_at.uncertainty_ticks = 1;
    assert_eq!(
        window.push(invalid_timestamp),
        Err(MeasurementWindowRefusal::InvalidTimestamp)
    );

    let mut wrong_uncertainty = sample(100, 3);
    wrong_uncertainty.uncertainty = Some(Quantity::new(1, QuantityUnit::Millimeter));
    assert_eq!(
        window.push(wrong_uncertainty),
        Err(MeasurementWindowRefusal::UncertaintyUnitMismatch)
    );
}

#[test]
fn reusable_window_has_an_exact_checked_form_contract() {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut catalog).unwrap();
    let source = include_str!("../../../forms/measurement-window/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "measurement-window", &catalog).unwrap();
    let gear = &authored.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), MEASUREMENT_COUNT_WINDOW_KIND);
    assert_eq!(
        gear.inputs[0].value_kind.as_str(),
        measurement_sample_type()
            .profile()
            .unwrap()
            .value_kind()
            .as_str()
    );
    assert_eq!(
        gear.outputs[0].value_kind.as_str(),
        measurement_window_type()
            .profile()
            .unwrap()
            .value_kind()
            .as_str()
    );
    assert_eq!(
        gear.inputs[0].temporal,
        conduit_core::PortTemporal::Flow { closes: true }
    );
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
}

#[test]
fn capacity_and_profile_ranges_refuse_before_storage_exists() {
    let mut invalid = profile(FullWindowPolicy::Reject);
    invalid.capacity = MAXIMUM_MEASUREMENT_WINDOW_SAMPLES + 1;
    assert_eq!(
        BoundedMeasurementWindow::new(invalid),
        Err(MeasurementWindowRefusal::CapacityOutOfBounds)
    );

    let mut invalid = profile(FullWindowPolicy::Reject);
    invalid.range.minimum = Quantity::new(1, QuantityUnit::Millivolt);
    invalid.range.maximum = Quantity::new(0, QuantityUnit::Millivolt);
    assert_eq!(
        BoundedMeasurementWindow::new(invalid),
        Err(MeasurementWindowRefusal::InvalidRange)
    );

    let mut invalid = profile(FullWindowPolicy::Reject);
    invalid.clock_basis.clear();
    assert_eq!(
        BoundedMeasurementWindow::new(invalid),
        Err(MeasurementWindowRefusal::InvalidClockProfile)
    );
}
