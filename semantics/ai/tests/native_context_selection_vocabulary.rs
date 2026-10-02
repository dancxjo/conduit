use conduit_ai::{
    ChunkIdentity, ContextOmission, ContextOmissionReason, ContextOrderingPolicy,
    ContextRedundancyPolicy, SelectedContextCost, SelectedContextRationale,
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
    assert_round_trip(SelectedContextCost::new(1024, 256, 8).unwrap());
    assert_round_trip(
        ContextOmission::new(
            ChunkIdentity::from_digest([7; 32]),
            ContextOmissionReason::TokenBudget,
        )
        .unwrap(),
    );
    assert!(!include_str!("../src/context_selection.rs")
        .contains(concat!("pub struct ", "SelectedContextCost")));
    assert!(!include_str!("../src/context_selection.rs")
        .contains(concat!("pub struct ", "ContextOmission")));

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
