use conduit_ai::{ContextSelectionRefusal, HouseContextRefusal, RerankingRefusal};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn selection_refusals_round_trip_through_native_types() {
    for value in [
        ContextSelectionRefusal::EmptyCandidates,
        ContextSelectionRefusal::MissingTemporalEvidence,
        ContextSelectionRefusal::NoSelectedContext,
    ] {
        assert_round_trip(value);
    }
    for value in [
        RerankingRefusal::EmptyCandidates,
        RerankingRefusal::UnexpectedObservation,
        RerankingRefusal::WorkBoundExceeded,
    ] {
        assert_round_trip(value);
    }
    for value in [
        HouseContextRefusal::EmptyUtterance,
        HouseContextRefusal::ContextByteLimitExceeded,
        HouseContextRefusal::InvalidOutputBound,
    ] {
        assert_round_trip(value);
    }
}
