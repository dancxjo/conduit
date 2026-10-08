#![cfg(feature = "semantic-bindings")]
use conduit_language::LanguageId;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{semantic::*, *};
#[allow(dead_code)]
mod fixture {
    use conduit_language::*;
    include!("common/occurrence_intent.rs");
}
#[path = "common_acoustic/common.rs"]
mod acoustic;
fn event() -> SpeechUtteranceIntentEvent {
    fixture::segment(10)
}
fn membership() -> SpeechOccurrenceMembership {
    SpeechOccurrenceMembership::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        fixture::token(10, fixture::BASIS),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn phone(ipa: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("phone/t".into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn selected() -> SpeechAllophoneChoiceState {
    SpeechAllophoneChoiceState::new(
        0,
        SpeechAllophoneChoiceOutcome::SelectedAllophone,
        SpeechContextDecision::Matched,
    )
    .unwrap()
}
fn time(n: u64, d: u64) -> conduit_audio::AudioTimeFraction {
    conduit_audio::AudioTimeFraction::new(d, n).unwrap()
}
fn t(
    start: conduit_audio::AudioTimeFraction,
    end: conduit_audio::AudioTimeFraction,
    frontier: conduit_audio::AudioTimeFraction,
    lookahead: conduit_audio::AudioTimeFraction,
) -> SpeechGestureTiming {
    SpeechGestureTiming::new(acoustic::anchor(), frontier, lookahead, end, start).unwrap()
}
fn timing() -> SpeechGestureTiming {
    t(time(1, 10), time(2, 10), time(0, 10), time(2, 10))
}
fn prepare(
    ipa: &str,
    t: &SpeechGestureTiming,
) -> Result<PreparedDeclaredPhoneGestures, SpeechGestureRefusal> {
    prepare_declared_phone_gestures(
        &event().encode().unwrap(),
        &membership().encode().unwrap(),
        &phone(ipa).encode().unwrap(),
        &selected().encode().unwrap(),
        &t.clone().encode().unwrap(),
    )
}
#[test]
fn original_identity_exact_phase_arithmetic_and_supported_unicode() {
    for ipa in ["i", "a", "u", "t", "d", "tʰ", "t͡ʃ"] {
        let receipt = prepare(ipa, &timing()).unwrap();
        assert_eq!(receipt.declared_phone().ipa(), ipa);
        assert_eq!(receipt.original_event(), &event());
        assert_eq!(receipt.original_timing(), &timing());
        for (gesture, frame) in receipt
            .gestures()
            .iter()
            .zip(receipt.admitted_canonical_frames())
        {
            assert_eq!(&SpeechAcousticGesture::decode(frame).unwrap(), gesture);
            assert_eq!(*gesture.start().denominator(), 50);
            assert_eq!(*gesture.end().denominator(), 50);
            match gesture.channel() {
                SpeechGestureChannel::Closure => assert_eq!(
                    (
                        *gesture.start().numerator_seconds(),
                        *gesture.end().numerator_seconds()
                    ),
                    (5, 8)
                ),
                SpeechGestureChannel::Release => assert_eq!(
                    (
                        *gesture.start().numerator_seconds(),
                        *gesture.end().numerator_seconds()
                    ),
                    (8, 9)
                ),
                SpeechGestureChannel::Aspiration | SpeechGestureChannel::Frication => assert_eq!(
                    (
                        *gesture.start().numerator_seconds(),
                        *gesture.end().numerator_seconds()
                    ),
                    (9, 10)
                ),
                _ => {}
            }
        }
        assert!(receipt.require_renderer_projection().is_err());
    }
    for ipa in ["tʰː", "tʃ", "ˈt", "n", "ɹ", "ã"] {
        assert!(matches!(
            prepare(ipa, &timing()),
            Err(SpeechGestureRefusal::UnsupportedSymbol)
        ));
    }
    let r = prepare("tʰ", &timing()).unwrap();
    assert!(r
        .gestures()
        .iter()
        .any(|g| matches!(g.channel(), SpeechGestureChannel::Aspiration)));
    let plain = prepare("t", &timing()).unwrap();
    assert!(!plain
        .gestures()
        .iter()
        .any(|g| matches!(g.channel(), SpeechGestureChannel::Aspiration)));
}
#[test]
fn independent_u128_bounds_and_frontier_basis_refusals() {
    for (start, end, den) in [
        (0, 1, 1),
        (1, 2, 10),
        (u32::MAX as u64 - 1, u32::MAX as u64, u32::MAX as u64),
    ] {
        let t = t(
            time(start, den),
            time(end, den),
            time(start, den),
            time(end, den),
        );
        let r = prepare("tʰ", &t).unwrap();
        let closure = r
            .gestures()
            .iter()
            .find(|g| matches!(g.channel(), SpeechGestureChannel::Closure))
            .unwrap();
        let expected = u128::from(start) * 5 + (u128::from(end) - u128::from(start)) * 3;
        assert_eq!(u128::from(*closure.end().numerator_seconds()), expected);
        assert!(expected <= u128::from(u64::MAX));
        assert_eq!(
            u128::from(*closure.end().denominator()),
            u128::from(den) * 5
        );
    }
    for t in [
        t(time(2, 10), time(1, 10), time(0, 10), time(2, 10)),
        t(time(1, 10), time(2, 10), time(2, 10), time(2, 10)),
        t(time(1, 10), time(2, 10), time(0, 10), time(1, 10)),
        t(time(1, 10), time(2, 20), time(0, 10), time(2, 10)),
        t(time(0, 1), time(u64::MAX, 1), time(0, 1), time(u64::MAX, 1)),
    ] {
        assert!(prepare("t", &t).is_err());
    }
}
#[test]
fn independent_overlapping_channels_and_same_channel_conflict() {
    let r = prepare("tʰ", &timing()).unwrap();
    let aspiration = r
        .gestures()
        .iter()
        .find(|g| matches!(g.channel(), SpeechGestureChannel::Aspiration))
        .unwrap();
    let formant = r
        .gestures()
        .iter()
        .find(|g| matches!(g.channel(), SpeechGestureChannel::FormantCenter))
        .unwrap();
    let safe = compare_gesture_overlap(
        &aspiration.clone().encode().unwrap(),
        &formant.clone().encode().unwrap(),
    )
    .unwrap();
    assert!(!safe.conflicts());
    safe.require_compatible().unwrap();
    let conflict = compare_gesture_overlap(
        &formant.clone().encode().unwrap(),
        &formant.clone().encode().unwrap(),
    )
    .unwrap();
    assert!(conflict.conflicts());
    assert!(conflict.require_compatible().is_err());
    for e in r.executions() {
        let program =
            conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                .unwrap();
        assert_eq!(
            program.evaluate(e.input_canonical()).unwrap(),
            e.output_canonical()
        );
    }
}
#[test]
fn original_six_spec_states_and_typed_corruption_refusals() {
    let states = [
        StressSpecification::known(SpeechStress::Primary).unwrap(),
        StressSpecification::unknown(),
        StressSpecification::unspecified(),
        StressSpecification::not_applicable(),
        StressSpecification::variable(
            BoundedSequence::try_from_iter([SpeechStress::Primary, SpeechStress::Secondary])
                .unwrap(),
        )
        .unwrap(),
        StressSpecification::gradient(
            SpeechConfidence::new(conduit_core::IeeeF32::from_value(0.5)).unwrap(),
            SpeechStress::Primary,
        )
        .unwrap(),
    ];
    for stress in states {
        let original = fixture::segment_stress(10, stress);
        let frame = original.clone().encode().unwrap();
        let r = prepare_declared_phone_gestures(
            &frame,
            &membership().encode().unwrap(),
            &phone("tʰ").encode().unwrap(),
            &selected().encode().unwrap(),
            &timing().encode().unwrap(),
        )
        .unwrap();
        assert_eq!(r.original_event(), &original);
        assert_eq!(r.original_canonical_frames()[0], frame);
    }
    let bad_membership = SpeechOccurrenceMembership::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        fixture::token(11, fixture::BASIS),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    assert!(prepare_declared_phone_gestures(
        &event().encode().unwrap(),
        &bad_membership.encode().unwrap(),
        &phone("t").encode().unwrap(),
        &selected().encode().unwrap(),
        &timing().encode().unwrap()
    )
    .is_err());
    let none = SpeechAllophoneChoiceState::new(
        0,
        SpeechAllophoneChoiceOutcome::None,
        SpeechContextDecision::Mismatched,
    )
    .unwrap();
    assert!(matches!(
        prepare_declared_phone_gestures(
            &event().encode().unwrap(),
            &membership().encode().unwrap(),
            &phone("t").encode().unwrap(),
            &none.encode().unwrap(),
            &timing().encode().unwrap()
        ),
        Err(SpeechGestureRefusal::UnselectedChoice)
    ));
    let mut corrupt = timing().encode().unwrap();
    corrupt.truncate(corrupt.len() - 1);
    assert!(prepare_declared_phone_gestures(
        &event().encode().unwrap(),
        &membership().encode().unwrap(),
        &phone("t").encode().unwrap(),
        &selected().encode().unwrap(),
        &corrupt
    )
    .is_err());
    let r = prepare("tʰ", &timing()).unwrap();
    println!(
        "gesture Type {} value {}; timing Type {} value {}; event Type {} value {}",
        SpeechAcousticGesture::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        r.admitted_canonical_frames()[0].len(),
        SpeechGestureTiming::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        r.original_canonical_frames()[4].len(),
        SpeechUtteranceIntentEvent::semantic_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        r.original_canonical_frames()[0].len()
    );
}
#[test]
fn exact_existing_audio_trajectory_carrier_retains_quantity_and_original() {
    let r = prepare("tʰ", &timing()).unwrap();
    for frame in r.admitted_canonical_frames() {
        let copied = gesture_to_audio_trajectory(frame).unwrap();
        let segment = &copied.trajectory().segments().as_slice()[0];
        assert_eq!(copied.original_canonical(), frame);
        assert_eq!(segment.left(), copied.original().quantity());
        assert_eq!(segment.right(), copied.original().quantity());
        assert_eq!(
            segment.start().numerator_seconds(),
            copied.original().start().numerator_seconds()
        );
        assert_eq!(
            segment.start().denominator(),
            copied.original().start().denominator()
        );
        assert_eq!(
            conduit_audio::AudioQuantityTrajectory::decode(copied.admitted_canonical()).unwrap(),
            *copied.trajectory()
        );
    }
}
#[test]
fn authored_anticipation_extension_and_foreign_anchor_refusal() {
    let r = prepare("tʰ", &timing()).unwrap();
    let source = &r.gestures()[0];
    let manual = SpeechAcousticGesture::new(
        source.anchor().clone(),
        SpeechGestureChannel::LaryngealVoicing,
        time(11, 50),
        0,
        source.occurrence().clone(),
        source.provenance().clone(),
        conduit_audio::AudioTrajectoryQuantity::amplitude(1, 1).unwrap(),
        SpeechGestureShape::Step,
        source.sources().clone(),
        time(1, 50),
    )
    .unwrap();
    let window = t(time(5, 50), time(10, 50), time(0, 50), time(12, 50));
    let receipt = prepare_authored_acoustic_gesture(
        &manual.clone().encode().unwrap(),
        &window.clone().encode().unwrap(),
    )
    .unwrap();
    assert_eq!(receipt.original_gesture(), &manual);
    assert_eq!(receipt.original_timing(), &window);
    let safe = compare_gesture_overlap(
        &manual.clone().encode().unwrap(),
        &r.gestures()
            .iter()
            .find(|g| matches!(g.channel(), SpeechGestureChannel::FormantCenter))
            .unwrap()
            .clone()
            .encode()
            .unwrap(),
    )
    .unwrap();
    safe.require_compatible().unwrap();
    let late = t(time(5, 50), time(10, 50), time(2, 50), time(12, 50));
    assert!(prepare_authored_acoustic_gesture(
        &manual.clone().encode().unwrap(),
        &late.encode().unwrap()
    )
    .is_err());
    let early = t(time(5, 50), time(10, 50), time(0, 50), time(10, 50));
    assert!(prepare_authored_acoustic_gesture(
        &manual.clone().encode().unwrap(),
        &early.encode().unwrap()
    )
    .is_err());
    let foreign = conduit_audio::AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(2).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let wrong = SpeechGestureTiming::new(
        foreign,
        time(0, 50),
        time(12, 50),
        time(10, 50),
        time(5, 50),
    )
    .unwrap();
    assert!(
        prepare_authored_acoustic_gesture(&manual.encode().unwrap(), &wrong.encode().unwrap())
            .is_err()
    );
}
