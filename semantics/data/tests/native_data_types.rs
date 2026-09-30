use conduit_core::{Quantity, QuantityUnit};
use conduit_data::{
    ClockRelationQuality, DataLoadTextTerminal, DataSaveTextTerminal, FullWindowPolicy,
    MeasurementPlotOverflowPolicy, MeasurementPlotRefusal, MeasurementSummaryRefusal,
    MeasurementThresholdRefusal, MeasurementThresholdState, MeasurementThresholdTransition,
    MeasurementWindowRefusal, QuantityMappingRefusal, QuantizationPolicy, RangePolicy,
    SampledSignalRefusal, SignalContinuity, TensorAxisRole, TensorElement,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
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
