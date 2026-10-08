#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;
fn source(revision: &str) -> LanguageSegmentRef {
    LanguageSegmentRef::phone(
        SpeechInventoryId::new("original-inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        2,
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechSegmentSequenceId::new("source-phones".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new("authored scope".into(), SpeechEvidenceSource::Manual, None)
        .unwrap()
}
#[test]
fn scope_and_original_event_membership_preserve_exact_material() {
    let original = source("r1");
    let foreign = source("r2");
    let scope = SpeechAcousticScope::new(
        "scope".into(),
        SpeechAcousticScopeKind::Word,
        BoundedSequence::try_from_iter([original.clone()]).unwrap(),
    )
    .unwrap();
    let matched = SpeechAcousticScopeSourceMatch::new(0, original.clone(), scope.clone()).unwrap();
    assert_eq!(matched.scope(), &scope);
    assert!(SpeechAcousticScopeSourceMatch::new(0, foreign.clone(), scope.clone()).is_err());
    assert!(SpeechAcousticScopeSourceMatch::new(1, original.clone(), scope).is_err());
    let sources = BoundedSequence::try_from_iter([original.clone()]).unwrap();
    let boundary = SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::unknown(),
        SpeechBoundarySpecification::unspecified(),
        provenance(),
        sources.clone(),
    )
    .unwrap();
    let segment = SpeechUtteranceIntentEvent::segment(
        LanguageSpeechTokenRef::new(
            SpeechInventoryId::new("target-inventory".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            0,
            SpeechSegmentRevisionId::new("target-revision".into()).unwrap(),
            SpeechSegmentSequenceId::new("target-phones".into()).unwrap(),
            SpeechUtteranceId::new("utterance".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::unknown(),
        PhonemeSpecification::unspecified(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::unknown(),
            SpeechCycleSpecification::unknown(),
            SpeechIntensitySpecification::unknown(),
        )
        .unwrap(),
        provenance(),
        sources,
        StressSpecification::unknown(),
        SpeechPositionSpecification::unknown(),
    )
    .unwrap();
    for event in [boundary, segment] {
        let admitted =
            SpeechAcousticEventSourceMatch::new(event.clone(), 0, original.clone()).unwrap();
        assert_eq!(admitted.event(), &event);
        assert_eq!(
            SpeechAcousticEventSourceMatch::decode(&admitted.clone().encode().unwrap()).unwrap(),
            admitted
        );
        assert!(SpeechAcousticEventSourceMatch::new(event.clone(), 0, foreign.clone()).is_err());
        assert!(SpeechAcousticEventSourceMatch::new(event, 1, original.clone()).is_err());
    }
}
#[test]
fn component_counts_refuse_over_profile_before_collection_allocation() {
    SpeechSharedAcousticComponentCounts::new(32, 32, 32, 32, 32, 32).unwrap();
    for values in [
        [33, 0, 0, 0, 0, 0],
        [0, 33, 0, 0, 0, 0],
        [0, 0, 33, 0, 0, 0],
        [0, 0, 0, 33, 0, 0],
        [0, 0, 0, 0, 33, 0],
        [0, 0, 0, 0, 0, 33],
    ] {
        assert!(SpeechSharedAcousticComponentCounts::new(
            values[0], values[1], values[2], values[3], values[4], values[5]
        )
        .is_err());
    }
}
