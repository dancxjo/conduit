use conduit_ai::{
    EntityBoundary, TemporalRetrievalIntent, TemporalRetrievalWindow, TemporalSource,
    TemporalValidity, TemporalWindowRelation, TransitionDirection,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

fn assert_clone_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn temporal_context_vocabularies_round_trip_through_their_native_types() {
    for source in [
        TemporalSource::Event,
        TemporalSource::ValidFrom,
        TemporalSource::ValidUntil,
        TemporalSource::Observed,
        TemporalSource::Recorded,
        TemporalSource::Ingested,
        TemporalSource::Retrieved,
    ] {
        assert_round_trip(source);
    }
    for boundary in [
        EntityBoundary::Created,
        EntityBoundary::FirstObserved,
        EntityBoundary::FirstUserMention,
        EntityBoundary::Born,
        EntityBoundary::Started,
        EntityBoundary::CurrentPhaseStarted,
        EntityBoundary::LastChanged,
    ] {
        assert_round_trip(boundary);
    }
    for direction in [
        TransitionDirection::IntoState,
        TransitionDirection::OutOfState,
    ] {
        assert_round_trip(direction);
    }
    for validity in [
        TemporalValidity::Current,
        TemporalValidity::Historical,
        TemporalValidity::Superseded,
        TemporalValidity::UnknownWhetherCurrent,
    ] {
        assert_round_trip(validity);
    }
    for relation in [
        TemporalWindowRelation::Before,
        TemporalWindowRelation::Within,
        TemporalWindowRelation::After,
        TemporalWindowRelation::Overlaps,
        TemporalWindowRelation::Unknown,
    ] {
        assert_round_trip(relation);
    }
}

#[test]
fn temporal_retrieval_intent_is_a_native_payload_rich_type() {
    for intent in [
        TemporalRetrievalIntent::EarliestEvidence,
        TemporalRetrievalIntent::LatestEvidence,
        TemporalRetrievalIntent::state_valid_at(u64::MAX).unwrap(),
        TemporalRetrievalIntent::transition(TransitionDirection::IntoState).unwrap(),
        TemporalRetrievalIntent::duration_since(EntityBoundary::Created).unwrap(),
        TemporalRetrievalIntent::EventOrdering,
        TemporalRetrievalIntent::evidence_within(TemporalRetrievalWindow::new(7, 7).unwrap())
            .unwrap(),
    ] {
        assert_clone_round_trip(intent);
    }
    assert!(TemporalRetrievalWindow::new(8, 7).is_err());
    assert!(!include_str!("../src/temporal_context.rs")
        .contains(concat!("pub enum ", "TemporalRetrievalIntent")));
}
