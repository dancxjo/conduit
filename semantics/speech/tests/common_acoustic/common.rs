#![allow(dead_code)]
use conduit_audio::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
use conduit_speech::*;
pub fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
pub fn time(n: u64, d: u64) -> AudioTimeFraction {
    AudioTimeFraction::new(d, n).unwrap()
}
pub fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "authored-common-target".into(),
        SpeechEvidenceSource::UserMarkup,
        Some("pinned-v1".into()),
    )
    .unwrap()
}
pub fn scope() -> SpeechAcousticScope {
    let source = LanguageSegmentRef::phoneme(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        conduit_language::LanguageId::new("language/test".into()).unwrap(),
        0,
        SpeechSegmentRevisionId::new("rev".into()).unwrap(),
        SpeechSegmentSequenceId::new("seq".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    SpeechAcousticScope::new(
        "phrase-1".into(),
        SpeechAcousticScopeKind::Phrase,
        BoundedSequence::try_from_iter(vec![source]).unwrap(),
    )
    .unwrap()
}
pub fn reproduce(executions: &[SpeechCommonAcousticExecution]) {
    for e in executions {
        assert_eq!(
            conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                .unwrap()
                .evaluate(e.input_canonical())
                .unwrap(),
            e.output_canonical()
        );
    }
}
pub fn probability_segment(
    start: u64,
    end: u64,
    value: SpeechProbabilitySpecification,
    interpolation: AudioTrajectoryInterpolation,
) -> SpeechProbabilityCurveSegment {
    SpeechProbabilityCurveSegment::new(time(end, 1), interpolation, time(start, 1), value).unwrap()
}
pub fn probability_curve(segments: Vec<SpeechProbabilityCurveSegment>) -> SpeechProbabilityCurve {
    SpeechProbabilityCurve::new(
        anchor(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        BoundedSequence::try_from_iter(segments).unwrap(),
    )
    .unwrap()
}
pub fn query(n: u64, d: u64) -> AudioTrajectoryQuery {
    AudioTrajectoryQuery::new(anchor(), AudioExactTimeOffset::new(d, n).unwrap()).unwrap()
}
