#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;

#[test]
fn phone_and_phoneme_identity_never_alias_or_decode_as_one_another() {
    let phone = PhoneId::new("en/t".into()).unwrap();
    let phoneme = PhonemeId::new("en/t".into()).unwrap();
    let phone_value = phone.into_structured().unwrap();
    let phoneme_value = phoneme.into_structured().unwrap();
    assert_ne!(phone_value.value_type(), phoneme_value.value_type());
    assert!(PhonemeId::from_structured(phone_value).is_err());
    assert!(PhoneId::from_structured(phoneme_value).is_err());
    assert!(PhoneId::new("".into()).is_err());
    assert!(PhoneId::new("x".repeat(65)).is_err());
}

#[test]
fn specification_states_roundtrip_without_turning_absence_into_a_phone() {
    let states = [
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::known(PhoneId::new("en/t".into()).unwrap()).unwrap(),
    ];
    for (index, state) in states.iter().enumerate() {
        let value = state.clone().into_structured().unwrap();
        assert_eq!(
            PhoneSpecification::from_structured(value.clone()).unwrap(),
            *state
        );
        for other in &states[..index] {
            assert_ne!(value, other.clone().into_structured().unwrap());
        }
    }
    assert!(PhoneSpecification::variable(BoundedSequence::new()).is_err());
}

#[test]
fn frame_spans_and_listening_ranges_have_distinct_units_and_enforce_order() {
    assert!(SpeechFrameSpan::new(10, 8000, 11, "synthesis/test".into()).is_err());
    let frames = SpeechFrameSpan::new(20, 8000, 10, "synthesis/test".into()).unwrap();
    let text = ListeningTextRange::new(20, 10).unwrap();
    assert_ne!(
        frames.into_structured().unwrap().value_type(),
        text.clone().into_structured().unwrap().value_type()
    );
    assert!(ListeningTextRange::new(3, 4).is_err());
    assert!(ListeningTimeRange::new(3, 4).is_err());
    assert!(AsrChunkedSimulation::new(100, 100).is_err());
}

#[test]
fn recognition_revisions_commits_and_cancellation_remain_different_events() {
    let segment = ListeningSegmentId::new("candidate/1".into()).unwrap();
    let partial = AsrRecognitionEvent::partial_hypothesis(
        None,
        ListeningTextRole::Recognition,
        segment.clone(),
        "hello".into(),
    )
    .unwrap();
    let revision = AsrRecognitionEvent::revised_hypothesis(
        None,
        ListeningTextRange::new(5, 0).unwrap(),
        ListeningTextRole::Recognition,
        segment.clone(),
        "hullo".into(),
    )
    .unwrap();
    let cancelled = AsrRecognitionEvent::hypothesis_cancelled(
        "withdrawn".into(),
        ListeningTextRole::Recognition,
        segment,
    )
    .unwrap();
    for event in [
        partial,
        revision,
        cancelled,
        AsrRecognitionEvent::Completed,
        AsrRecognitionEvent::EndOfStream,
    ] {
        let value = event.clone().into_structured().unwrap();
        assert_eq!(AsrRecognitionEvent::from_structured(value).unwrap(), event);
    }
}

#[test]
fn translations_allow_many_to_many_links_but_do_not_invent_empty_matches() {
    let reference = |text: &str, revision: &str, start, end| {
        LanguageSegmentRef::text(
            LanguageTextSegmentKind::Word,
            SpeechLanguageId::new("en".into()).unwrap(),
            ListeningTextRange::new(end, start).unwrap(),
            LanguageTextRevisionId::new(revision.into()).unwrap(),
            LanguageTextId::new(text.into()).unwrap(),
        )
        .unwrap()
    };
    let sources = BoundedSequence::try_from_iter([
        reference("source", "r1", 0, 3),
        reference("source", "r1", 4, 7),
    ])
    .unwrap();
    let targets = BoundedSequence::try_from_iter([reference("target", "r2", 0, 8)]).unwrap();
    let aligned = TranslationCorrespondence::aligned(sources, targets.clone()).unwrap();
    let value = aligned.clone().into_structured().unwrap();
    assert_eq!(
        TranslationCorrespondence::from_structured(value).unwrap(),
        aligned
    );
    assert!(TranslationCorrespondence::aligned(BoundedSequence::new(), targets.clone()).is_err());
    let insertion =
        TranslationCorrespondence::inserted("grammatical expansion".into(), targets).unwrap();
    assert_ne!(
        insertion.into_structured().unwrap(),
        aligned.into_structured().unwrap()
    );
}

#[test]
fn acoustic_evidence_keeps_feature_values_optional_time_and_float_precision() {
    use conduit_core::{IeeeF32, IeeeF64};
    let score = SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap();
    assert!(SpeechConfidence::new(IeeeF32::from_value(-0.1)).is_err());
    assert!(SpeechConfidence::new(IeeeF32::from_value(f32::NAN)).is_err());
    let feature = SpeechFeatureValue::number(IeeeF64::from_value(1800.125)).unwrap();
    let observation = SpeechAcousticObservation::new(
        score,
        SpeechAcousticCueId::new("formant/f2".into()).unwrap(),
        None,
        FeatureSpecification::known(feature).unwrap(),
    )
    .unwrap();
    let value = observation.clone().into_structured().unwrap();
    assert_eq!(
        SpeechAcousticObservation::from_structured(value).unwrap(),
        observation
    );
    let seconds =
        SpeechSegmentSpan::seconds(IeeeF64::from_value(0.125), IeeeF64::from_value(0.001)).unwrap();
    assert!(SpeechFrameSpan::from_structured(seconds.into_structured().unwrap()).is_err());
}
