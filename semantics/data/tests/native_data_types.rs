use conduit_core::{Quantity, QuantityUnit};
use conduit_data::{
    ClockRelation, ClockRelationQuality, DataLoadTextTerminal, DataSaveTextTerminal,
    FullWindowPolicy, MathScalarRefusal, MeasurementHysteresisProfile,
    MeasurementPlotOverflowPolicy, MeasurementPlotRefusal, MeasurementRange, MeasurementSample,
    MeasurementSummaryRefusal, MeasurementThresholdPolicy, MeasurementThresholdRefusal,
    MeasurementThresholdState, MeasurementThresholdTransition, MeasurementWindowRefusal,
    NormalizedQuantityRefusal, QuantityMappingRefusal, QuantizationPolicy, RangePolicy,
    SampledSignalRefusal, ScalarComparison, SignalContinuity, SignalStart, TensorAxisRole,
    TensorElement,
};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn measurement_range_and_hysteresis_profile_are_native_records() {
    let range = MeasurementRange {
        minimum: Quantity::new(-20, QuantityUnit::Celsius),
        maximum: Quantity::new(50, QuantityUnit::Celsius),
    };
    assert_round_trip(range);

    let policy = MeasurementThresholdPolicy::new(
        Quantity::new(18, QuantityUnit::Celsius),
        Quantity::new(24, QuantityUnit::Celsius),
    )
    .unwrap();
    assert_round_trip(MeasurementHysteresisProfile {
        policy,
        initial_state: MeasurementThresholdState::Below,
    });
}

#[test]
fn measurement_sample_uses_the_native_temporal_instant() {
    let sample = MeasurementSample {
        value: Quantity::new(21, QuantityUnit::Celsius),
        observed_at: NativeTemporalInstant::new(
            "sensor-clock".into(),
            1,
            NativeTemporalScale::Milliseconds,
            42,
            0,
        )
        .unwrap(),
        uncertainty: Some(Quantity::new(1, QuantityUnit::Celsius)),
    };
    assert_owned_round_trip(sample);
}

#[test]
fn sampled_signal_start_uses_the_native_temporal_instant() {
    assert_owned_round_trip(SignalStart::at_sample(u64::MAX));
    let instant = NativeTemporalInstant::new(
        "signal-clock".into(),
        1,
        NativeTemporalScale::Nanoseconds,
        42,
        0,
    )
    .unwrap();
    assert_owned_round_trip(SignalStart::instant(instant).unwrap());
}

#[test]
fn measurement_threshold_policy_round_trips_exact_quantities() {
    let policy = MeasurementThresholdPolicy::new(
        Quantity::new(i64::MIN, QuantityUnit::Millivolt),
        Quantity::new(i64::MAX, QuantityUnit::Millivolt),
    )
    .unwrap();
    assert_round_trip(policy);
    assert_eq!(
        *policy.lower(),
        Quantity::new(i64::MIN, QuantityUnit::Millivolt)
    );
    assert_eq!(
        *policy.upper(),
        Quantity::new(i64::MAX, QuantityUnit::Millivolt)
    );
}

fn assert_owned_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn measurement_refusals_round_trip_through_exact_native_types() {
    for refusal in [
        MeasurementWindowRefusal::CapacityOutOfBounds,
        MeasurementWindowRefusal::InvalidClockProfile,
        MeasurementWindowRefusal::InvalidRange,
        MeasurementWindowRefusal::InvalidTimestamp,
        MeasurementWindowRefusal::UnitMismatch,
        MeasurementWindowRefusal::UncertaintyUnitMismatch,
        MeasurementWindowRefusal::NegativeUncertainty,
        MeasurementWindowRefusal::ClockMismatch,
        MeasurementWindowRefusal::TimestampRegression,
        MeasurementWindowRefusal::OutOfRange,
        MeasurementWindowRefusal::Full,
        MeasurementWindowRefusal::DiscardCountOverflow,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        MeasurementPlotRefusal::InvalidPointCapacity,
        MeasurementPlotRefusal::EmptyWindow,
        MeasurementPlotRefusal::Full,
        MeasurementPlotRefusal::DegenerateValueRange,
        MeasurementPlotRefusal::ArithmeticOverflow,
        MeasurementPlotRefusal::InvalidProjection,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        MeasurementSummaryRefusal::EmptyWindow,
        MeasurementSummaryRefusal::UnitMismatch,
        MeasurementSummaryRefusal::ArithmeticOverflow,
        MeasurementSummaryRefusal::InexactMean,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        MeasurementThresholdRefusal::PolicyUnitMismatch,
        MeasurementThresholdRefusal::InvalidPolicyOrder,
        MeasurementThresholdRefusal::SummaryUnitMismatch,
    ] {
        assert_round_trip(refusal);
    }
}

#[test]
fn sampled_signal_terminal_round_trips_through_its_exact_native_type() {
    for value in [
        SampledSignalRefusal::InvalidClock,
        SampledSignalRefusal::InvalidStart,
        SampledSignalRefusal::InvalidCadence,
        SampledSignalRefusal::InvalidContinuity,
        SampledSignalRefusal::EmptySignal,
        SampledSignalRefusal::TensorInvalid,
        SampledSignalRefusal::SampleCountMismatch,
        SampledSignalRefusal::MissingSampleAxis,
        SampledSignalRefusal::WindowOutOfBounds,
        SampledSignalRefusal::TemporalOverflow,
        SampledSignalRefusal::IncompatibleSignals,
        SampledSignalRefusal::NoncontiguousSignals,
        SampledSignalRefusal::TooManyParts,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn sampled_signal_continuity_owns_its_identity_bounds() {
    assert_owned_round_trip(SignalContinuity::Continuous);
    assert_owned_round_trip(SignalContinuity::discontinuous("capture-gap".into()).unwrap());
    assert_owned_round_trip(SignalContinuity::clock_reset("clock/prior".into()).unwrap());

    assert!(SignalContinuity::discontinuous(String::new()).is_err());
    assert!(SignalContinuity::clock_reset("x".repeat(129)).is_err());
}

#[test]
fn data_text_terminals_round_trip_through_exact_native_types() {
    for value in [
        DataSaveTextTerminal::ValueTooLarge,
        DataSaveTextTerminal::GenerationCapacityExhausted,
        DataSaveTextTerminal::ByteCapacityExhausted,
        DataSaveTextTerminal::WrongContentKind,
    ] {
        assert_round_trip(value);
    }
    for value in [
        DataLoadTextTerminal::MalformedReference,
        DataLoadTextTerminal::WrongContentKind,
        DataLoadTextTerminal::WrongAccessClass,
        DataLoadTextTerminal::ExpiringGeneration,
        DataLoadTextTerminal::ItemExtent,
        DataLoadTextTerminal::GenerationNotRetained,
        DataLoadTextTerminal::ExtentMismatch,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn measurement_policy_variants_round_trip_through_exact_native_types() {
    for value in [FullWindowPolicy::Reject, FullWindowPolicy::DropOldest] {
        assert_round_trip(value);
    }
    for value in [
        MeasurementPlotOverflowPolicy::Reject,
        MeasurementPlotOverflowPolicy::EvenlySpaced,
    ] {
        assert_round_trip(value);
    }
    for value in [
        MeasurementThresholdState::Below,
        MeasurementThresholdState::Above,
    ] {
        assert_round_trip(value);
    }
    for value in [
        MeasurementThresholdTransition::RoseAbove,
        MeasurementThresholdTransition::FellBelow,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn quantity_mapping_vocabulary_round_trips_through_exact_native_types() {
    for value in [RangePolicy::Refuse, RangePolicy::Clamp] {
        assert_round_trip(value);
    }
    for value in [QuantizationPolicy::Exact, QuantizationPolicy::Nearest] {
        assert_round_trip(value);
    }
    for value in [
        QuantityMappingRefusal::InvalidRange,
        QuantityMappingRefusal::OutOfRange,
        QuantityMappingRefusal::Inexact,
        QuantityMappingRefusal::Overflow,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn scalar_comparisons_round_trip_through_their_exact_native_type() {
    for value in [
        ScalarComparison::Less,
        ScalarComparison::LessOrEqual,
        ScalarComparison::Equal,
        ScalarComparison::NotEqual,
        ScalarComparison::GreaterOrEqual,
        ScalarComparison::Greater,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn math_scalar_refusals_round_trip_through_their_exact_native_type() {
    for value in [
        MathScalarRefusal::InvalidConfiguration,
        MathScalarRefusal::Overflow,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn normalized_quantity_refusals_round_trip_through_their_exact_native_type() {
    for value in [
        NormalizedQuantityRefusal::MalformedOrWrongType,
        NormalizedQuantityRefusal::IncompatibleUnit,
        NormalizedQuantityRefusal::OutOfDomain,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn tensor_elements_round_trip_through_their_exact_native_type() {
    for value in [
        TensorElement::I8,
        TensorElement::U8,
        TensorElement::I16,
        TensorElement::I24,
        TensorElement::U16,
        TensorElement::I32,
        TensorElement::U32,
        TensorElement::I64,
        TensorElement::U64,
        TensorElement::F32,
        TensorElement::F64,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn tensor_axis_roles_keep_bounded_other_meaning_in_the_native_type() {
    let other = TensorAxisRole::other("articulatory-coordinate".into()).unwrap();
    let TensorAxisRole::Other(payload) = &other else {
        panic!("other axis retains its exact identity")
    };
    assert_eq!(payload.identity(), "articulatory-coordinate");
    let structured = other.clone().into_structured().unwrap();
    assert_eq!(TensorAxisRole::from_structured(structured).unwrap(), other);

    assert!(TensorAxisRole::other(String::new()).is_err());
    assert!(TensorAxisRole::other("x".repeat(65)).is_err());

    for role in [
        TensorAxisRole::Batch,
        TensorAxisRole::Time,
        TensorAxisRole::Feature,
        TensorAxisRole::Sensor,
        TensorAxisRole::SpatialCoordinate,
        TensorAxisRole::Frequency,
        TensorAxisRole::Channel,
    ] {
        let structured = role.clone().into_structured().unwrap();
        assert_eq!(TensorAxisRole::from_structured(structured).unwrap(), role);
    }
}

#[test]
fn clock_relation_quality_keeps_duration_meaning_in_the_native_type() {
    let estimated =
        ClockRelationQuality::estimated(Quantity::new(3, QuantityUnit::Microsecond)).unwrap();
    let ClockRelationQuality::Estimated(payload) = &estimated else {
        panic!("estimated quality retains its maximum error")
    };
    assert_eq!(
        *payload.maximum_error(),
        Quantity::new(3, QuantityUnit::Microsecond)
    );
    let structured = estimated.clone().into_structured().unwrap();
    assert_eq!(
        ClockRelationQuality::from_structured(structured).unwrap(),
        estimated
    );

    let exact = ClockRelationQuality::Exact;
    let structured = exact.clone().into_structured().unwrap();
    assert_eq!(
        ClockRelationQuality::from_structured(structured).unwrap(),
        exact
    );

    assert!(ClockRelationQuality::estimated(Quantity::new(1, QuantityUnit::Celsius)).is_err());
}

#[test]
fn clock_relation_round_trips_exact_native_bounds() {
    let boundary = "x".repeat(128);
    let relation = ClockRelation::new(
        boundary.clone(),
        ClockRelationQuality::Exact,
        u64::MAX,
        boundary.clone(),
        1,
        u64::MAX,
        "target".into(),
        u64::MAX,
    )
    .unwrap();
    assert_owned_round_trip(relation);

    assert!(ClockRelation::new(
        "x".repeat(129),
        ClockRelationQuality::Exact,
        0,
        "source".into(),
        1,
        0,
        "target".into(),
        1,
    )
    .is_err());
    assert!(ClockRelation::new(
        "identity".into(),
        ClockRelationQuality::Exact,
        0,
        "source".into(),
        0,
        0,
        "target".into(),
        1,
    )
    .is_err());
}
