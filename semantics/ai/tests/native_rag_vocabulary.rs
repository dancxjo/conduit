use conduit_ai::{
    ContextSelectionRationale, ContextTruncationReason, GroundedAnswerDisposition,
    GroundingDisposition, SourceSpanUnit,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn rag_vocabularies_round_trip_through_their_native_types() {
    for unit in [SourceSpanUnit::Bytes, SourceSpanUnit::Items] {
        assert_round_trip(unit);
    }
    for rationale in [
        ContextSelectionRationale::ExactMatch,
        ContextSelectionRationale::SemanticCandidate,
        ContextSelectionRationale::MetadataMatch,
        ContextSelectionRationale::TemporalMatch,
        ContextSelectionRationale::BoundaryEvidence,
        ContextSelectionRationale::ConflictPreserved,
    ] {
        assert_round_trip(rationale);
    }
    for reason in [
        ContextTruncationReason::ByteBudget,
        ContextTruncationReason::TokenBudget,
        ContextTruncationReason::ItemBudget,
    ] {
        assert_round_trip(reason);
    }
    for disposition in [
        GroundingDisposition::Supported,
        GroundingDisposition::InsufficientEvidence,
        GroundingDisposition::ConflictingEvidence,
    ] {
        assert_round_trip(disposition);
    }
    for disposition in [
        GroundedAnswerDisposition::Supported,
        GroundedAnswerDisposition::PartiallySupported,
        GroundedAnswerDisposition::InsufficientEvidence,
        GroundedAnswerDisposition::ConflictingEvidence,
    ] {
        assert_round_trip(disposition);
    }
}
