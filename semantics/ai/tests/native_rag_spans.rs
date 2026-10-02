use conduit_ai::{
    AnswerSpan, CitationIndices, ContextBudgetCost, ContextSelectionOutcome,
    ContextTruncationReason, GroundedClaim, RetrievalScore, SourceSpan, SourceSpanUnit,
};
use conduit_form::rust_binding::{BoundedSequence, NativeRustBinding};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn rag_spans_are_native_nonempty_intervals() {
    round_trip(SourceSpan::new(SourceSpanUnit::Bytes, 0, u64::MAX).unwrap());
    round_trip(SourceSpan::new(SourceSpanUnit::Items, 7, 8).unwrap());
    round_trip(AnswerSpan::new(0, u32::MAX).unwrap());

    assert!(SourceSpan::new(SourceSpanUnit::Bytes, 0, 0).is_err());
    assert!(SourceSpan::new(SourceSpanUnit::Items, 9, 8).is_err());
    assert!(AnswerSpan::new(1, 1).is_err());
    assert!(AnswerSpan::new(2, 1).is_err());

    let rust = include_str!("../src/rag_semantics.rs");
    assert!(!rust.contains(concat!("pub struct ", "SourceSpan")));
    assert!(!rust.contains(concat!("pub struct ", "AnswerSpan")));
}

#[test]
fn retrieval_score_and_context_budget_are_native_values() {
    round_trip(RetrievalScore::new(i64::MIN).unwrap());
    round_trip(RetrievalScore::new(i64::MAX).unwrap());
    round_trip(ContextBudgetCost::new(1, 0).unwrap());
    round_trip(ContextBudgetCost::new(0, u32::MAX).unwrap());
    assert!(ContextBudgetCost::new(0, 0).is_err());

    let rust = include_str!("../src/rag_semantics.rs");
    assert!(!rust.contains(concat!("pub struct ", "RetrievalScore")));
    assert!(!rust.contains(concat!("pub struct ", "ContextBudgetCost")));
}

#[test]
fn context_selection_outcome_owns_positive_omission_truth() {
    round_trip(ContextSelectionOutcome::Complete);
    round_trip(
        ContextSelectionOutcome::truncated(1_024, ContextTruncationReason::TokenBudget).unwrap(),
    );
    assert!(ContextSelectionOutcome::truncated(0, ContextTruncationReason::ItemBudget).is_err());
    assert!(
        ContextSelectionOutcome::truncated(1_025, ContextTruncationReason::ItemBudget).is_err()
    );
    assert!(!include_str!("../src/rag_semantics.rs")
        .contains(concat!("pub enum ", "ContextSelectionOutcome")));
}

#[test]
fn grounded_claim_owns_a_nonempty_bounded_citation_list() {
    let indices = CitationIndices::new(
        BoundedSequence::try_from_iter(0..128).expect("exactly 128 indices fit"),
    )
    .unwrap();
    round_trip(GroundedClaim::new(AnswerSpan::new(1, 2).unwrap(), indices).unwrap());
    assert!(CitationIndices::new(BoundedSequence::new()).is_err());

    let rust = include_str!("../src/rag_semantics.rs");
    assert!(!rust.contains(concat!("pub struct ", "GroundedClaim")));
    assert!(!rust.contains("ClaimWithoutCitation"));
}
