use conduit_audio::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
#[allow(dead_code)]
#[path = "correspondence.rs"]
pub mod original;

pub fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
pub fn time(n: u64) -> AudioExactTimeOffset {
    AudioExactTimeOffset::new(1, n).unwrap()
}
pub fn carrier() -> SpeechAcousticCarrierBasis {
    SpeechAcousticCarrierBasis::new(
        anchor(),
        original::alignment(original::groups())
            .inventory_basis()
            .clone(),
        original::source(),
    )
    .unwrap()
}
pub fn word(start: u32, end: u32) -> LanguageTextSegmentRef {
    LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Word,
        original::source().language().clone(),
        LanguageTextRange::new(end, start).unwrap(),
        original::source().revision_id().clone(),
        original::source().text_id().clone(),
    )
    .unwrap()
}
pub fn sources() -> Vec<SpeechSyllableSource> {
    [(0, 3, word(0, 3)), (3, 7, word(4, 9))]
        .into_iter()
        .map(|(start, end, word)| {
            let count = end - start;
            let syllable = SpeechPlannedSyllableIntent::new(
                original::basis("phones"),
                SpeechSyllableId::new(format!("supplied-{start}")).unwrap(),
                Some(0),
                BoundedSequence::try_from_iter((0..count).map(|n| {
                    if n == 0 {
                        SpeechSyllablePosition::Nucleus
                    } else {
                        SpeechSyllablePosition::Coda
                    }
                }))
                .unwrap(),
                BoundedSequence::try_from_iter(
                    (start..end).map(|n| original::reference("phones", n)),
                )
                .unwrap(),
                original::provenance(),
                None,
                StressSpecification::unspecified(),
            )
            .unwrap();
            let ipa = conduit_speech::ipa_phonetic::phonetic_from_ipa(
                "ɾ".repeat(count as usize),
                original::provenance(),
            )
            .unwrap();
            SpeechSyllableSource::new(
                ipa.transcription().clone(),
                BoundedSequence::try_from_iter(0..count as u64).unwrap(),
                syllable,
                original::profile().variety().clone(),
                word,
            )
            .unwrap()
        })
        .collect()
}
pub fn boundary() -> SpeechPositionedBoundary {
    SpeechPositionedBoundary::new(
        original::basis("phones"),
        SpeechBoundaryKind::Phrase,
        7,
        original::source(),
    )
    .unwrap()
}
pub fn all_scopes() -> Vec<SpeechAcousticScope> {
    vec![
        SpeechAcousticScope::phone(
            SpeechPhoneAcousticScope::new(
                anchor(),
                carrier().inventory_basis().clone(),
                original::reference("phones", 0),
                original::source(),
            )
            .unwrap(),
        )
        .unwrap(),
        SpeechAcousticScope::syllable(
            SpeechAcousticSyllableScope::new(
                carrier(),
                SpeechSyllableId::new("supplied-0".into()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
        SpeechAcousticScope::word(SpeechAcousticWordScope::new(carrier(), word(0, 3)).unwrap())
            .unwrap(),
        SpeechAcousticScope::phrase(
            SpeechAcousticPhraseScope::new(
                boundary(),
                carrier(),
                7,
                original::reference("phones", 0),
            )
            .unwrap(),
        )
        .unwrap(),
        SpeechAcousticScope::utterance(carrier()).unwrap(),
    ]
}
pub fn target(
    scope: SpeechAcousticScope,
    role: SpeechFrequencyRole,
    identity: &str,
    source_scope: &str,
    priority: u16,
    hz: u64,
    span: (u64, u64),
) -> SpeechFrequencyGestureTarget {
    let (start, end) = span;
    let provenance = AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "explicit target".into(),
        None,
    )
    .unwrap();
    let trajectory = AudioQuantityTrajectory::new(
        anchor(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance,
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            time(end),
            AudioTrajectoryInterpolation::Step,
            AudioTrajectoryQuantity::frequency(1, hz).unwrap(),
            AudioTrajectoryQuantity::frequency(1, hz).unwrap(),
            time(start),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    SpeechFrequencyGestureTarget::new(
        AudioScopedControlTrajectory::new(
            identity.into(),
            AudioTrajectoryDomain::Frequency,
            priority,
            source_scope.into(),
            trajectory,
        )
        .unwrap(),
        role,
        scope,
    )
    .unwrap()
}
pub fn set(
    targets: Vec<SpeechFrequencyGestureTarget>,
    conflict: AudioControlConflictPolicy,
) -> SpeechFrequencyGestureSet {
    SpeechFrequencyGestureSet::new(
        conflict,
        AudioControlGapPolicy::RefuseAnyGap,
        BoundedSequence::try_from_iter(targets).unwrap(),
    )
    .unwrap()
}
pub fn observation(
    scope: SpeechAcousticScope,
    role: SpeechFrequencyRole,
    error: SpeechFrequencyErrorSpecification,
) -> SpeechFrequencyObservationEvidence {
    SpeechFrequencyObservationEvidence::new(
        AudioTrajectoryQuery::new(anchor(), time(1)).unwrap(),
        original::confidence(),
        error,
        AudioFrequencyHz::new(1, 110).unwrap(),
        original::provenance(),
        role,
        scope,
        time(1),
    )
    .unwrap()
}

pub fn gapped_target(scope: SpeechAcousticScope) -> SpeechFrequencyGestureTarget {
    let base = target(
        scope,
        SpeechFrequencyRole::Fundamental,
        "F0",
        "gapped",
        0,
        120,
        (0, 3),
    );
    let t = base.control().trajectory();
    let endpoint = t.segments()[0].left();
    let segments = [(0, 1), (2, 3)].map(|(start, end)| {
        AudioTrajectorySegment::new(
            time(end),
            AudioTrajectoryInterpolation::Step,
            endpoint.clone(),
            endpoint.clone(),
            time(start),
        )
        .unwrap()
    });
    let trajectory = AudioQuantityTrajectory::new(
        t.anchor().clone(),
        *t.endpoints(),
        *t.outside(),
        t.provenance().clone(),
        BoundedSequence::try_from_iter(segments).unwrap(),
    )
    .unwrap();
    let c = base.control();
    SpeechFrequencyGestureTarget::new(
        AudioScopedControlTrajectory::new(
            c.control_identity().clone(),
            *c.domain(),
            *c.priority(),
            c.source_scope().clone(),
            trajectory,
        )
        .unwrap(),
        *base.role(),
        base.scope().clone(),
    )
    .unwrap()
}
