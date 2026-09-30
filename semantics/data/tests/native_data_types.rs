use conduit_data::{
    FullWindowPolicy, MeasurementPlotOverflowPolicy, MeasurementThresholdState,
    MeasurementThresholdTransition, SampledSignalRefusal,
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
