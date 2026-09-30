use conduit_ai::{
    EvaluationDisposition, HumanAssessmentDisposition, LearnedLifecycleRefusal, PromotionDecision,
    PromotionTerminal, RollbackTerminal, ShadowTerminal,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().expect("native value encodes");
    assert_eq!(
        structured.value_type(),
        &T::semantic_type().expect("native semantic Type checks")
    );
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn learned_lifecycle_vocabulary_is_native_conduitese() {
    assert_round_trip(ShadowTerminal::CandidateProviderLost);
    assert_round_trip(HumanAssessmentDisposition::SupportsCandidate);
    assert_round_trip(EvaluationDisposition::Disagreement);
    assert_round_trip(PromotionDecision::Approved);
    assert_round_trip(PromotionTerminal::CommitUnknown);
    assert_round_trip(RollbackTerminal::RolledBack);
    assert_round_trip(LearnedLifecycleRefusal::InvalidPlanTransition);
}
