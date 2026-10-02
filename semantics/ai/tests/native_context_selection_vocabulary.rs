use conduit_ai::{
    ChunkIdentity, ContextOmission, ContextOmissionReason, ContextOmissions, ContextOrderingPolicy,
    ContextRedundancyPolicy, ContextSelectionDisposition, SelectedContextCost,
    SelectedContextRationale,
};
use conduit_form::rust_binding::{BoundedSequence, NativeRustBinding};

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
    assert!(!include_str!("../src/context_selection.rs")
        .contains(concat!("pub enum ", "ContextSelectionDisposition")));

    let complete = ContextSelectionDisposition::Complete;
    assert_eq!(
        ContextSelectionDisposition::from_structured(complete.clone().into_structured().unwrap())
            .unwrap(),
        complete
    );
    let omitted = ContextSelectionDisposition::omitted(
        ContextOmissions::new(
            BoundedSequence::try_from_iter([ContextOmission::new(
                ChunkIdentity::from_digest([9; 32]),
                ContextOmissionReason::WorkBudget,
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ContextSelectionDisposition::from_structured(omitted.clone().into_structured().unwrap())
            .unwrap(),
        omitted
    );
    assert!(ContextOmissions::new(BoundedSequence::new()).is_err());

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
