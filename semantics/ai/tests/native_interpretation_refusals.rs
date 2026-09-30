use conduit_ai::{
    InterpretationInvalidity, TemporalEvidenceSelectionRefusal, TemporalInterpretationRefusal,
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
fn interpretation_refusals_round_trip_through_native_types() {
    for value in [
        InterpretationInvalidity::EmptyEvidence,
        InterpretationInvalidity::InvalidConfidence,
        InterpretationInvalidity::InvalidTemporalContext,
    ] {
        assert_round_trip(value);
    }
    for value in [
        TemporalEvidenceSelectionRefusal::EmptyCandidates,
        TemporalEvidenceSelectionRefusal::ReferenceMismatch,
        TemporalEvidenceSelectionRefusal::MissingSourceTime,
    ] {
        assert_round_trip(value);
    }
    for value in [
        TemporalInterpretationRefusal::InvalidRequest,
        TemporalInterpretationRefusal::UnresolvedAmbiguity,
        TemporalInterpretationRefusal::ArithmeticOverflow,
    ] {
        assert_round_trip(value);
    }
}
