use conduit_core::{Quantity, QuantityUnit, TemporalInstant, TemporalScale};
use conduit_data::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn summary(value: i64, unit: QuantityUnit, ticks: u64) -> MeasurementSummary {
    let instant = TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "sensor-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    MeasurementSummary {
        unit,
        sample_count: 3,
        first_observed_at: instant.clone(),
        last_observed_at: instant,
        minimum: Quantity::new(value, unit),
        maximum: Quantity::new(value, unit),
        range: Quantity::new(0, unit),
        mean: Quantity::new(value, unit),
    }
}

fn policy() -> MeasurementThresholdPolicy {
    MeasurementThresholdPolicy {
        lower: Quantity::new(40, QuantityUnit::Millivolt),
        upper: Quantity::new(60, QuantityUnit::Millivolt),
    }
}

#[test]
fn hysteresis_transitions_only_at_the_explicit_boundaries() {
    let mut threshold =
        MeasurementHysteresis::new(policy(), MeasurementThresholdState::Below).unwrap();
    let below_band = threshold
        .evaluate(&summary(50, QuantityUnit::Millivolt, 1))
        .unwrap();
    assert_eq!(below_band.state, MeasurementThresholdState::Below);
    assert_eq!(below_band.transition, None);

    let rose = threshold
        .evaluate(&summary(60, QuantityUnit::Millivolt, 2))
        .unwrap();
    assert_eq!(
        rose.transition,
        Some(MeasurementThresholdTransition::RoseAbove)
    );
    let above_band = threshold
        .evaluate(&summary(50, QuantityUnit::Millivolt, 3))
        .unwrap();
    assert_eq!(above_band.state, MeasurementThresholdState::Above);
    assert_eq!(above_band.transition, None);

    let fell = threshold
        .evaluate(&summary(40, QuantityUnit::Millivolt, 4))
        .unwrap();
    assert_eq!(
        fell.transition,
        Some(MeasurementThresholdTransition::FellBelow)
    );
}

#[test]
fn invalid_policy_and_summary_units_refuse_distinctly() {
    let mixed = MeasurementThresholdPolicy {
        lower: Quantity::new(40, QuantityUnit::Millivolt),
        upper: Quantity::new(60, QuantityUnit::Millimeter),
    };
    assert_eq!(
        MeasurementHysteresis::new(mixed, MeasurementThresholdState::Below),
        Err(MeasurementThresholdRefusal::PolicyUnitMismatch)
    );
    let reversed = MeasurementThresholdPolicy {
        lower: Quantity::new(60, QuantityUnit::Millivolt),
        upper: Quantity::new(40, QuantityUnit::Millivolt),
    };
    assert_eq!(
        MeasurementHysteresis::new(reversed, MeasurementThresholdState::Below),
        Err(MeasurementThresholdRefusal::InvalidPolicyOrder)
    );
    let mut threshold =
        MeasurementHysteresis::new(policy(), MeasurementThresholdState::Below).unwrap();
    assert_eq!(
        threshold.evaluate(&summary(50, QuantityUnit::Millimeter, 1)),
        Err(MeasurementThresholdRefusal::SummaryUnitMismatch)
    );
}

#[test]
fn threshold_is_a_reusable_form_independent_of_presentation() {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_summary_catalog(&mut startup, &mut catalog).unwrap();
    install_measurement_threshold_catalog(&mut startup, &mut catalog).unwrap();
    let source = include_str!("../../../forms/measurement-threshold/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "measurement-threshold", &catalog).unwrap();
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        MEASUREMENT_HYSTERESIS_KIND
    );
    assert!(!source.contains("presentation"));
    assert!(!source.contains("indicator"));
}
