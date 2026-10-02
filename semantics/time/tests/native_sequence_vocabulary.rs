use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{PatternComparisonRefusal, SequenceNormalizationRefusal};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn sequence_refusals_round_trip_through_their_exact_native_types() {
    for refusal in [
        SequenceNormalizationRefusal::Malformed,
        SequenceNormalizationRefusal::Empty,
        SequenceNormalizationRefusal::TooManyValues,
        SequenceNormalizationRefusal::ZeroDuration,
    ] {
        assert_round_trip(refusal);
    }

    for refusal in [
        PatternComparisonRefusal::Malformed,
        PatternComparisonRefusal::UnsupportedMetric,
        PatternComparisonRefusal::ToleranceOutOfRange,
        PatternComparisonRefusal::AlgorithmMismatch,
        PatternComparisonRefusal::LengthMismatch,
    ] {
        assert_round_trip(refusal);
    }
}
