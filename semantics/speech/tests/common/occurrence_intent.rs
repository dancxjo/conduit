use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub fn token(ordinal: u32, basis: [&str; 5]) -> LanguageSpeechTokenRef {
    LanguageSpeechTokenRef::new(
        SpeechInventoryId::new(basis[0].into()).unwrap(),
        LanguageId::new(basis[1].into()).unwrap(),
        ordinal,
        SpeechSegmentRevisionId::new(basis[2].into()).unwrap(),
        SpeechSegmentSequenceId::new(basis[3].into()).unwrap(),
        SpeechUtteranceId::new(basis[4].into()).unwrap(),
    )
    .unwrap()
}
pub const BASIS: [&str; 5] = ["inventory", "en", "revision", "sequence", "utterance"];
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "occurrence fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn sources() -> BoundedSequence<LanguageSegmentRef, 8> {
    BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        LanguageId::new("en".into()).unwrap(),
        LanguageTextRange::new(1, 0).unwrap(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        LanguageTextId::new("source".into()).unwrap(),
    )
    .unwrap()])
    .unwrap()
}
pub fn segment(ordinal: u32) -> SpeechUtteranceIntentEvent {
    segment_with(token(ordinal, BASIS))
}
pub fn segment_with(occurrence: LanguageSpeechTokenRef) -> SpeechUtteranceIntentEvent {
    segment_parts(
        occurrence,
        StressSpecification::known(SpeechStress::Primary).unwrap(),
    )
}
pub fn segment_stress(ordinal: u32, stress: StressSpecification) -> SpeechUtteranceIntentEvent {
    segment_parts(token(ordinal, BASIS), stress)
}
fn segment_parts(
    occurrence: LanguageSpeechTokenRef,
    stress: StressSpecification,
) -> SpeechUtteranceIntentEvent {
    SpeechUtteranceIntentEvent::segment(
        occurrence,
        PhoneSpecification::known(PhoneId::new("phone/t".into()).unwrap()).unwrap(),
        PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::unknown(),
            SpeechCycleSpecification::unknown(),
            SpeechIntensitySpecification::unknown(),
        )
        .unwrap(),
        provenance(),
        sources(),
        stress,
        SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
    )
    .unwrap()
}
pub fn boundary(kind: SpeechBoundarySpecification) -> SpeechUtteranceIntentEvent {
    SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::unknown(),
        kind,
        provenance(),
        sources(),
    )
    .unwrap()
}
pub fn intent(
    events: impl IntoIterator<Item = SpeechUtteranceIntentEvent>,
) -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        SpeechInventoryId::new(BASIS[0].into()).unwrap(),
        LanguageId::new(BASIS[1].into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new(BASIS[2].into()).unwrap(),
        SpeechUtteranceId::new(BASIS[4].into()).unwrap(),
    )
    .unwrap()
}
