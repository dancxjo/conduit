use conduit_data::{
    DataLoadTextTerminal, DataSaveTextTerminal, FullWindowPolicy, MeasurementPlotOverflowPolicy,
    MeasurementThresholdState, MeasurementThresholdTransition, SampledSignalRefusal,
    TensorAxisRole, TensorElement,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
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
