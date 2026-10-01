use conduit_ai::{
    ContextOmissionReason, ContextOrderingPolicy, ContextRedundancyPolicy, SelectedContextRationale,
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
fn context_selection_vocabularies_round_trip_through_their_native_types() {
    for policy in [
        ContextRedundancyPolicy::KeepAll,
        ContextRedundancyPolicy::OnePerReviewedGroup,
    ] {
        assert_round_trip(policy);
    }
    for policy in [
        ContextOrderingPolicy::Reranked,
        ContextOrderingPolicy::ChronologicalOldestFirst,
    ] {
        assert_round_trip(policy);
    }
    for rationale in [
        SelectedContextRationale::Reranked,
        SelectedContextRationale::TemporalChronology,
    ] {
        assert_round_trip(rationale);
    }
    for reason in [
        ContextOmissionReason::ReviewedRedundancy,
        ContextOmissionReason::ItemBudget,
        ContextOmissionReason::ByteBudget,
        ContextOmissionReason::TokenBudget,
        ContextOmissionReason::WorkBudget,
    ] {
        assert_round_trip(reason);
    }
}
