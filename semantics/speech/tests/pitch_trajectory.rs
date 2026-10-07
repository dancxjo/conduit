#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
use conduit_speech::Renderer;

fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new("timing fixture".into(), SpeechEvidenceSource::Manual, None)
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
fn segment() -> SpeechUtteranceIntentEvent {
    SpeechUtteranceIntentEvent::segment(
        LanguageSpeechTokenRef::new(
            SpeechInventoryId::new("inventory".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            0,
            SpeechSegmentRevisionId::new("revision".into()).unwrap(),
            SpeechSegmentSequenceId::new("phones".into()).unwrap(),
            SpeechUtteranceId::new("utterance".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::known(PhoneId::new("opaque/aa".into()).unwrap()).unwrap(),
        PhonemeSpecification::unspecified(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(5, 1).unwrap(),
            SpeechCycleSpecification::known(100, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        provenance(),
        sources(),
        StressSpecification::unknown(),
        SpeechPositionSpecification::unspecified(),
    )
    .unwrap()
}
fn utterance(events: Vec<SpeechUtteranceIntentEvent>) -> SpeechUtteranceIntent {
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn binding() -> SpeechFormantBoundaryBinding {
    SpeechFormantBoundaryBinding::new(SpeechBoundaryKind::Word, SpeechFormantBoundary::Word)
        .unwrap()
}
fn profile(bindings: Vec<SpeechFormantBoundaryBinding>) -> SpeechFormantBoundaryProfile {
    SpeechFormantBoundaryProfile::new(BoundedSequence::try_from_iter(bindings).unwrap()).unwrap()
}
fn pcm(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut buffer = [0; 128];
    let mut result = Vec::new();
    while !renderer.is_complete() {
        let n = renderer.render(&mut buffer[..block]).unwrap();
        result.extend_from_slice(&buffer[..n]);
    }
    result
}

#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod fixture;
use conduit_speech::intent_realization::*;
fn definition() -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("opaque/aa".into()).unwrap(),
        "ɑ".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn inventory() -> SpeechInventory {
    fixture::inventory("inventory", "en", vec![definition()])
}
fn voice() -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
        "fixture".into(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechFormantPhoneBinding::new(
            definition(),
            EnglishPhone::Aa,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
use conduit_language::LanguageProsodyPitch;
use conduit_speech::pitch_trajectory::*;
fn planned(source: &SpeechUtteranceIntent) -> SpeechPlannedSegmentIntent {
    let SpeechUtteranceIntentEvent::Segment(value) = &source.events().as_slice()[0] else {
        panic!()
    };
    SpeechPlannedSegmentIntent::new(
        value.occurrence().clone(),
        value.phone().clone(),
        value.phoneme().clone(),
        value.prosody().clone(),
        value.provenance().clone(),
        value.sources().clone(),
        value.stress().clone(),
        value.word_position().clone(),
    )
    .unwrap()
}
fn admission(
    source: &SpeechUtteranceIntent,
    pitch: LanguageProsodyPitch,
    end_denominator: u64,
) -> SpeechSegmentPitchAdmission {
    let trajectory = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(5, 1).unwrap(),
        SpeechFundamentalCycle::new(end_denominator, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    SpeechSegmentPitchAdmission::new(pitch, planned(source), trajectory).unwrap()
}
#[test]
fn exact_endpoint_and_midpoint_receipts_share_physical_time_across_cadences() {
    let source = utterance(vec![segment()]);
    let curve = admission(&source, LanguageProsodyPitch::Rising, 200);
    let at8 = sample_at_frame(&curve, 800, 8000).unwrap();
    let at16 = sample_at_frame(&curve, 1600, 16000).unwrap();
    assert_eq!(*at8.whole_q8(), 15360);
    assert_eq!(*at16.whole_q8(), 30720);
    assert_eq!(*at8.remainder_numerator(), 0);
    assert_eq!(*sample_at_frame(&curve, 0, 8000).unwrap().whole_q8(), 20480);
    assert_eq!(
        *sample_at_frame(&curve, 1600, 8000).unwrap().whole_q8(),
        10240
    );
    assert!(sample_at_frame(&curve, 1601, 8000).is_err());
    assert!(sample_at_frame(&curve, 0, 0).is_err());
}
#[test]
fn native_direction_duration_and_start_cycle_refuse_substitution() {
    let source = utterance(vec![segment()]);
    let curve = admission(&source, LanguageProsodyPitch::Rising, 200);
    assert!(SpeechSegmentPitchAdmission::new(
        LanguageProsodyPitch::Falling,
        planned(&source),
        curve.trajectory().clone()
    )
    .is_err());
    let wrong = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(10, 1).unwrap(),
        SpeechFundamentalCycle::new(200, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    assert!(SpeechSegmentPitchAdmission::new(
        LanguageProsodyPitch::Rising,
        planned(&source),
        wrong
    )
    .is_err());
    let foreign = utterance(vec![segment(), segment()]);
    let offers = [
        OfferedSegmentPitch {
            event: 0,
            admission: &curve,
        },
        OfferedSegmentPitch {
            event: 0,
            admission: &curve,
        },
    ];
    assert!(matches!(
        prepare_utterance_pitch(&foreign, &offers),
        Err(PitchRefusal::Duplicate { event: 0 })
    ));
    assert!(prepare_utterance_pitch(
        &source,
        &[OfferedSegmentPitch {
            event: 1,
            admission: &curve
        }]
    )
    .is_err());
}
#[test]
fn actual_formant_contour_is_block_invariant_and_staged_cursor_can_be_discarded() {
    let source = utterance(vec![segment()]);
    let curve = admission(&source, LanguageProsodyPitch::Rising, 200);
    let pitch = prepare_utterance_pitch(
        &source,
        &[OfferedSegmentPitch {
            event: 0,
            admission: &curve,
        }],
    )
    .unwrap();
    let inv = inventory();
    let voice = voice();
    let boundaries = profile(vec![binding()]);
    let realized = prepare_intent_realization(&source, &inv, &voice, &boundaries).unwrap();
    let curved = pcm(pitch.renderer(&realized).unwrap(), 63);
    assert_eq!(curved, pcm(pitch.renderer(&realized).unwrap(), 1));
    assert_eq!(curved.len(), 1600);
    assert_ne!(curved, pcm(realized.renderer().unwrap(), 63));
    let original = pitch.renderer(&realized).unwrap();
    let mut staged = original;
    let mut block = [0; 63];
    staged.render(&mut block).unwrap();
    assert_eq!(original.rendered_frames(), 0);
    assert_eq!(staged.rendered_frames(), 63);
    assert_eq!(pcm(original, 128), curved);
}
#[path = "../../language/tests/common/prosody.rs"]
mod linguistic_fixture;
#[test]
fn rich_linguistic_choice_profile_and_exact_source_survive_acceptance_and_refusal() {
    use conduit_language::{prosody::prepare_rich_prosody, LanguageProsodyProfile};
    use conduit_speech::linguistic_prosody::LinguisticProsodyBasis;
    let fixture = linguistic_fixture::fixture("Travis Hello", 0, 1);
    let profile = LanguageProsodyProfile::new(
        fixture.profile.fallback().clone(),
        fixture.profile.identity().clone(),
        fixture.profile.language().clone(),
        fixture.profile.provenance().clone(),
        linguistic_fixture::choice(
            conduit_language::LanguageProsodyBoundary::MinorPhrase,
            conduit_language::LanguageProsodyProminence::Prominent,
            LanguageProsodyPitch::Rising,
        ),
    )
    .unwrap();
    let rich =
        prepare_rich_prosody(&fixture.lexical, 0, fixture.discourse.fact(), &profile).unwrap();
    let reference = conduit_language::language_source_occurrence(
        rich.requested().source().material(),
        rich.requested().token().span(),
        LanguageTextSegmentKind::Word,
    )
    .unwrap();
    let native_source = LanguageSegmentRef::text(
        *reference.kind(),
        reference.language().clone(),
        reference.range().clone(),
        reference.revision_id().clone(),
        reference.text_id().clone(),
    )
    .unwrap();
    let source = utterance(vec![segment()]);
    let original = planned(&source);
    let exact = SpeechPlannedSegmentIntent::new(
        original.occurrence().clone(),
        original.phone().clone(),
        original.phoneme().clone(),
        original.prosody().clone(),
        original.provenance().clone(),
        BoundedSequence::try_from_iter([native_source]).unwrap(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap();
    let binding = SpeechLinguisticProsodyBinding::new(
        rich.accepted().choice().clone(),
        "voice/1".into(),
        profile.language().clone(),
        profile.identity().clone(),
        provenance(),
        SpeechLinguisticProsodyRealization::new(
            SpeechDurationSpecification::known(100, 3).unwrap(),
            SpeechBoundarySpecification::Known(SpeechBoundaryKind::Phrase),
            exact.prosody().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let trajectory = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(5, 1).unwrap(),
        SpeechFundamentalCycle::new(200, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    let prepared = prepare_linguistic_pitch(
        LinguisticProsodyBasis::Rich(&rich),
        &binding,
        &exact,
        &trajectory,
    )
    .unwrap();
    assert_eq!(
        prepared.accepted().pitch().requested(),
        &LanguageProsodyPitch::Rising
    );
    assert_eq!(prepared.requested().source(), rich.requested().source());
    let refused = prepare_linguistic_pitch(
        LinguisticProsodyBasis::Rich(&rich),
        &binding,
        &original,
        &trajectory,
    )
    .err()
    .unwrap();
    assert!(matches!(refused.reason(), PitchRefusal::Source { .. }));
    assert_eq!(
        refused.requested().choice().pitch(),
        &LanguageProsodyPitch::Rising
    );
    assert!(core::ptr::eq(refused.trajectory(), &trajectory));
    let foreign_binding = SpeechLinguisticProsodyBinding::new(
        binding.choice().clone(),
        binding.identity().clone(),
        binding.language().clone(),
        "foreign/profile".into(),
        binding.provenance().clone(),
        binding.realization().clone(),
    )
    .unwrap();
    let refused = prepare_linguistic_pitch(
        LinguisticProsodyBasis::Rich(&rich),
        &foreign_binding,
        &exact,
        &trajectory,
    )
    .err()
    .unwrap();
    assert!(matches!(refused.reason(), PitchRefusal::Native(_)));
    assert!(core::ptr::eq(refused.binding(), &foreign_binding));
    let falling = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(5, 1).unwrap(),
        SpeechFundamentalCycle::new(50, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    let refused = prepare_linguistic_pitch(
        LinguisticProsodyBasis::Rich(&rich),
        &binding,
        &exact,
        &falling,
    )
    .err()
    .unwrap();
    assert!(matches!(refused.reason(), PitchRefusal::Native(_)));
    assert_eq!(refused.binding(), &binding);
}
#[test]
fn exact_quantity_overflow_refuses_without_partial_realization() {
    let source = utterance(vec![segment()]);
    let original = planned(&source);
    let huge = SpeechPlannedSegmentIntent::new(
        original.occurrence().clone(),
        original.phone().clone(),
        original.phoneme().clone(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(1, u64::MAX).unwrap(),
            original.prosody().fundamental_cycle().clone(),
            original.prosody().relative_intensity().clone(),
        )
        .unwrap(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap();
    let trajectory = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(1, u64::MAX).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    let admission =
        SpeechSegmentPitchAdmission::new(LanguageProsodyPitch::Level, huge, trajectory).unwrap();
    assert!(sample_at_frame(&admission, 0, 8000).is_err());
}
