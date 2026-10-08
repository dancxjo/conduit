#![cfg(feature = "semantic-bindings")]
//! Actual opaque allophone-choice composition gate; run in the Speech owner.
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    allophone_selection::*, contextual_gestures::*, declared_context::ExplicitAllophoneContext,
    semantic::*,
};
#[allow(dead_code)]
mod fixture {
    use conduit_language::*;
    include!("common/occurrence_intent.rs");
}
fn phone(id: &str, ipa: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new(id.into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn inventory() -> SpeechInventory {
    let rule = SpeechPhonemeAllophone::new(
        BoundedSequence::new(),
        SpeechConfidence::new(conduit_core::IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        PhoneId::new("phone/t-asp".into()).unwrap(),
        Some("demo/stressed-onset-aspiration/1".into()),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([rule]).unwrap(),
        Some(PhoneId::new("phone/t".into()).unwrap()),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/t".into()).unwrap(),
        "t".into(),
        BoundedSequence::try_from_iter([
            PhoneId::new("phone/t".into()).unwrap(),
            PhoneId::new("phone/t-asp".into()).unwrap(),
        ])
        .unwrap(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone("phone/t", "t"), phone("phone/t-asp", "tʰ")])
            .unwrap(),
    )
    .unwrap()
}
fn intent(stress: StressSpecification) -> SpeechUtteranceIntent {
    let SpeechUtteranceIntentEvent::Segment(base) = fixture::segment_stress(10, stress) else {
        unreachable!()
    };
    fixture::intent([SpeechUtteranceIntentEvent::segment(
        base.occurrence().clone(),
        PhoneSpecification::unspecified(),
        base.phoneme().clone(),
        base.prosody().clone(),
        base.provenance().clone(),
        base.sources().clone(),
        base.stress().clone(),
        base.word_position().clone(),
    )
    .unwrap()])
}
fn timing() -> SpeechGestureTiming {
    let anchor = conduit_audio::AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(1).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let time = |n| conduit_audio::AudioTimeFraction::new(10, n).unwrap();
    SpeechGestureTiming::new(anchor, time(0), time(2), time(2), time(1)).unwrap()
}
#[test]
fn actual_context_selects_phone_without_rewriting_original_phoneme() {
    let inventory = inventory();
    let policy = SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let syllable =
        SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap();
    let prosody = SpeechProsodicContextSpecification::unspecified();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    for (stress, expected, aspirates) in [
        (SpeechStress::Primary, "tʰ", true),
        (SpeechStress::Unstressed, "t", false),
    ] {
        let intent = intent(StressSpecification::known(stress).unwrap());
        let original = intent.clone();
        let choice = select_intent_allophone(
            &intent,
            0,
            &inventory,
            &policy,
            ExplicitAllophoneContext {
                syllable_position: &syllable,
                prosodic_context: &prosody,
                careful_style: &style,
            },
        )
        .unwrap();
        let receipt = prepare_contextual_phone_gestures(&choice, &timing()).unwrap();
        assert_eq!(receipt.choice().occurrence().intent(), &original);
        assert_eq!(receipt.choice().phoneme().notation(), "t");
        assert_eq!(receipt.lowered().declared_phone().ipa(), expected);
        assert_eq!(
            receipt
                .lowered()
                .gestures()
                .iter()
                .any(|g| matches!(g.channel(), SpeechGestureChannel::Aspiration)),
            aspirates
        );
    }
    let intent = intent(StressSpecification::known(SpeechStress::Primary).unwrap());
    let missing = SpeechSyllablePositionSpecification::unknown();
    let choice = select_intent_allophone(
        &intent,
        0,
        &inventory,
        &policy,
        ExplicitAllophoneContext {
            syllable_position: &missing,
            prosodic_context: &prosody,
            careful_style: &style,
        },
    )
    .unwrap();
    assert!(matches!(
        prepare_contextual_phone_gestures(&choice, &timing()),
        Err(ContextualGestureRefusal::NoSelectedPhone)
    ));
}
