use conduit_ai::{
    EntityBoundary, TemporalSource, TemporalValidity, TemporalWindowRelation, TransitionDirection,
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
