use conduit_core::{Quantity, TemporalInstant, TemporalScale, Unit};
use conduit_data::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn summary(value: i64, unit: Unit, ticks: u64) -> MeasurementSummary {
    let instant = TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "sensor-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    MeasurementSummary {
        sample_count: 3,
        first_observed_at: instant.clone().try_into().unwrap(),
        last_observed_at: instant.try_into().unwrap(),
        minimum: Quantity::new(value, unit),
        maximum: Quantity::new(value, unit),
        range: Quantity::new(0, unit),
        mean: Quantity::new(value, unit),
    }
}

fn policy() -> MeasurementThresholdPolicy {
    MeasurementThresholdPolicy::new(
        Quantity::new(40, Unit::Millivolt),
        Quantity::new(60, Unit::Millivolt),
    )
    .unwrap()
}

#[test]
fn hysteresis_transitions_only_at_the_explicit_boundaries() {
    let mut threshold =
        MeasurementHysteresis::new(policy(), MeasurementThresholdState::Below).unwrap();
    let below_band = threshold
        .evaluate(&summary(50, Unit::Millivolt, 1))
        .unwrap();
    assert_eq!(below_band.state, MeasurementThresholdState::Below);
    assert_eq!(below_band.transition, None);

    let rose = threshold
        .evaluate(&summary(60, Unit::Millivolt, 2))
        .unwrap();
    assert_eq!(
        rose.transition,
        Some(MeasurementThresholdTransition::RoseAbove)
    );
    let above_band = threshold
        .evaluate(&summary(50, Unit::Millivolt, 3))
        .unwrap();
    assert_eq!(above_band.state, MeasurementThresholdState::Above);
    assert_eq!(above_band.transition, None);

    let fell = threshold
        .evaluate(&summary(40, Unit::Millivolt, 4))
        .unwrap();
    assert_eq!(
        fell.transition,
        Some(MeasurementThresholdTransition::FellBelow)
    );
}

#[test]
fn invalid_policy_and_summary_units_refuse_distinctly() {
    let mixed = MeasurementThresholdPolicy::new(
        Quantity::new(40, Unit::Millivolt),
        Quantity::new(60, Unit::Millimeter),
    )
    .unwrap();
    assert_eq!(
        MeasurementHysteresis::new(mixed, MeasurementThresholdState::Below),
        Err(MeasurementThresholdRefusal::PolicyUnitMismatch)
    );
    let reversed = MeasurementThresholdPolicy::new(
        Quantity::new(60, Unit::Millivolt),
        Quantity::new(40, Unit::Millivolt),
    )
    .unwrap();
    assert_eq!(
        MeasurementHysteresis::new(reversed, MeasurementThresholdState::Below),
        Err(MeasurementThresholdRefusal::InvalidPolicyOrder)
    );
    let mut threshold =
        MeasurementHysteresis::new(policy(), MeasurementThresholdState::Below).unwrap();
    assert_eq!(
        threshold.evaluate(&summary(50, Unit::Millimeter, 1)),
        Err(MeasurementThresholdRefusal::SummaryUnitMismatch)
    );
}

#[test]
fn threshold_is_a_reusable_plot_independent_of_presentation() {
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
        expand_canonical_plot_for_authoring(&checked, "measurement-threshold", &catalog).unwrap();
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        MEASUREMENT_HYSTERESIS_KIND
    );
    assert_eq!(
        authored.expanded.gears[0].inputs[0].port_id.as_str(),
        "profile"
    );
}

#[test]
fn hysteresis_profile_and_decision_payloads_round_trip_exactly() {
    let profile = MeasurementHysteresisProfile {
        policy: policy(),
        initial_state: MeasurementThresholdState::Below,
    };
    assert_eq!(
        decode_measurement_hysteresis_profile(
            &encode_measurement_hysteresis_profile(profile).unwrap()
        ),
        Ok(profile)
    );
    assert_eq!(
        encode_measurement_hysteresis_profile(profile).unwrap(),
        vec![1, 7, 40, 0, 0, 0, 0, 0, 0, 0, 7, 60, 0, 0, 0, 0, 0, 0, 0, 0,]
    );
    let mut hysteresis = MeasurementHysteresis::new(profile.policy, profile.initial_state).unwrap();
    let decision = hysteresis
        .evaluate(&summary(60, Unit::Millivolt, 2))
        .unwrap();
    assert_eq!(
        decode_measurement_threshold_decision(
            &encode_measurement_threshold_decision(&decision).unwrap()
        ),
        Ok(decision)
    );
}
