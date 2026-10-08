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
fn prepared_fixture(
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

fn policy() -> SpeechGreetingLossPolicy {
    SpeechGreetingLossPolicy::AcceptLateralRhoticAndStepDiphthongApproximationV2
}
fn prepare_v2(
    ipa: &str,
    timing: &SpeechGestureTiming,
    loss: SpeechGreetingLossPolicy,
) -> Result<PreparedGreetingPhoneGestures, SpeechGestureRefusal> {
    prepare_greeting_phone_gestures(
        &event().encode().unwrap(),
        &membership().encode().unwrap(),
        &phone(ipa).encode().unwrap(),
        &selected().encode().unwrap(),
        &timing.clone().encode().unwrap(),
        loss,
    )
}
fn grid(rate: u64) -> conduit_audio::AudioSampleRateBasis {
    conduit_audio::AudioSampleRateBasis::new(
        acoustic::anchor(),
        conduit_audio::AudioFrameQuantization::Floor,
        rate,
    )
    .unwrap()
}
#[test]
fn caller_selected_typed_losses_and_full_originals_survive() {
    use conduit_core::projection::*;
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    for ipa in ["l", "ɹ", "oʊ"] {
        assert!(matches!(
            prepare_v2(
                ipa,
                &timing,
                SpeechGreetingLossPolicy::RefuseUnsupportedMechanisms
            ),
            Err(SpeechGestureRefusal::Admission(_))
        ));
        let receipt = prepare_v2(ipa, &timing, policy()).unwrap();
        assert_eq!(receipt.lowered().declared_phone(), &phone(ipa));
        assert_eq!(receipt.lowered().original_event(), &event());
        assert_eq!(receipt.lowered().original_timing(), &timing);
        assert_eq!(
            receipt.lowered().profile_identity(),
            "speech/authored-greeting-approximation/2"
        );
        assert_eq!(receipt.selected_policy(), &policy());
        let view = GreetingApproximationProjection::new(&receipt);
        let report = view.report().unwrap();
        assert_eq!(
            report.summary().disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
        );
        assert!(report.require_exact().is_err());
        for e in receipt.lowered().executions() {
            assert_eq!(
                conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                e.output_canonical()
            );
        }
    }
    for ipa in ["h", "ə", "ɛ", "æ", "ɪ", "v", "s"] {
        let receipt = prepare_v2(
            ipa,
            &timing,
            SpeechGreetingLossPolicy::RefuseUnsupportedMechanisms,
        )
        .unwrap();
        let view = GreetingApproximationProjection::new(&receipt);
        let report = view.report().unwrap();
        assert_eq!(
            report.summary().disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::Exact)
        );
        assert!(receipt.second_start().is_none());
    }
    let legacy = prepare_v2(
        "i",
        &timing,
        SpeechGreetingLossPolicy::RefuseUnsupportedMechanisms,
    )
    .unwrap();
    assert_eq!(
        legacy.lowered().profile_identity(),
        "speech/authored-acoustic-gesture-demo/1"
    );
    assert!(prepared_fixture("ə", &timing).is_err());
    for ipa in ["n", "m", "ɾ", "r", "ɫ", "o", "oː", "ˈə"] {
        assert!(prepare_v2(ipa, &timing, policy()).is_err());
    }
}
#[test]
fn source_authored_diphthong_half_targets_and_phase_preserving_filter_reset() {
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    let original = prepare_v2("oʊ", &timing, policy()).unwrap();
    let half = original.second_start().unwrap();
    assert_eq!((*half.numerator_seconds(), *half.denominator()), (1, 20));
    // A nonintegral number of cycles at the half boundary exposes phase resets.
    let cycle = conduit_audio::AudioCycleDuration::new(8000, 37)
        .unwrap()
        .encode()
        .unwrap();
    let plan = prepare_greeting_renderer(&original, &grid(8000).encode().unwrap(), &cycle).unwrap();
    assert_eq!(
        *plan
            .boundary_projection()
            .unwrap()
            .result()
            .raw()
            .whole_frames(),
        400
    );
    assert_eq!(
        *plan.first_target().coefficients()[0]
            .original()
            .resonator()
            .center()
            .numerator_hz(),
        500
    );
    assert_eq!(
        *plan.second_target().unwrap().coefficients()[0]
            .original()
            .resonator()
            .center()
            .numerator_hz(),
        300
    );
    assert_eq!(
        plan.first_target().selected_target_evaluations()[0]
            .original()
            .segments()
            .as_slice()
            .len(),
        2
    );
    assert_eq!(
        plan.first_target().selected_target_evaluations()[0].selected_segment(),
        0
    );
    assert_eq!(
        plan.second_target().unwrap().selected_target_evaluations()[0].selected_segment(),
        1
    );
    let mut cursor = plan.cursor();
    let mut previous_phase = 0;
    let mut count = 0;
    while let Some(frame) = plan.next(&mut cursor).unwrap() {
        let rendered = frame.rendered();
        let n = rendered.frame();
        assert_eq!(rendered.input_phase_q8(), previous_phase);
        assert_eq!(rendered.output_phase_q8(), ((n + 1) % 37) * 256);
        assert_eq!(frame.is_second_target(), n >= 400);
        if n == 400 {
            assert_eq!(rendered.input_phase_q8(), 30 * 256);
            assert_eq!(frame.target_selection_and_reset_executions().len(), 2);
        } else {
            assert_eq!(frame.target_selection_and_reset_executions().len(), 1);
        }
        for e in frame.target_selection_and_reset_executions() {
            assert_eq!(
                conduit_plot::PortableExpressionProgram::from_canonical_hex(e.source_program_hex())
                    .unwrap()
                    .evaluate(e.input_canonical())
                    .unwrap(),
                e.output_canonical()
            );
        }
        previous_phase = rendered.output_phase_q8();
        count += 1;
    }
    assert_eq!(count, 800);
    assert!(prepare_speech_gesture_renderer(
        original.lowered(),
        &grid(8000).encode().unwrap(),
        &cycle
    )
    .is_err());
}
#[test]
fn every_required_greeting_phone_reaches_actual_dsp_at_declared_16khz() {
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    let basis = grid(16000).encode().unwrap();
    let cycle = conduit_audio::AudioCycleDuration::new(200, 1)
        .unwrap()
        .encode()
        .unwrap();
    let mut waves = Vec::new();
    for ipa in ["h", "ə", "ɛ", "l", "oʊ", "t", "ɹ", "æ", "v", "ɪ", "s"] {
        let original = prepare_v2(ipa, &timing, policy()).unwrap();
        let plan = prepare_greeting_renderer(&original, &basis, &cycle).unwrap();
        let mut cursor = plan.cursor();
        let mut samples = Vec::new();
        while let Some(frame) = plan.next(&mut cursor).unwrap() {
            samples.push(i16::try_from(frame.rendered().sample()).unwrap());
        }
        assert_eq!(samples.len(), 1600);
        assert!(samples.iter().any(|s| *s != 0), "{ipa}");
        waves.push(samples);
    }
    assert_ne!(waves[0], waves[1]);
    assert_ne!(waves[3], waves[6]);
    assert_ne!(waves[8], waves[10]);
    if let Some(path) = std::env::var_os("CONDUIT_GREETING_PROFILE_PROOF_WAV") {
        let samples = waves.iter().flatten().copied().collect::<Vec<_>>();
        let bytes = u32::try_from(samples.len() * 2).unwrap();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16000_u32.to_le_bytes());
        wav.extend_from_slice(&32000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for s in samples {
            wav.extend_from_slice(&s.to_le_bytes())
        }
        std::fs::write(path, wav).unwrap();
    }
}

#[test]
fn reviewed_features_survive_and_unresolved_or_foreign_values_refuse() {
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    let value = SpeechFeatureValue::category("voiceless".into()).unwrap();
    let states = [
        FeatureSpecification::known(value.clone()).unwrap(),
        FeatureSpecification::unknown(),
        FeatureSpecification::unspecified(),
        FeatureSpecification::not_applicable(),
        FeatureSpecification::variable(BoundedSequence::try_from_iter([value.clone()]).unwrap())
            .unwrap(),
        FeatureSpecification::gradient(
            SpeechConfidence::new(conduit_core::IeeeF32::from_value(0.5)).unwrap(),
            value,
        )
        .unwrap(),
        FeatureSpecification::known(SpeechFeatureValue::category("voiced".into()).unwrap())
            .unwrap(),
    ];
    for (index, state) in states.into_iter().enumerate() {
        let feature = |name: &str, value| {
            SpeechFeature::new(
                SpeechFeatureId::new(format!("reviewed/greeting/{name}")).unwrap(),
                value,
            )
            .unwrap()
        };
        let bundle = SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([
                feature(
                    "segment-class",
                    FeatureSpecification::known(
                        SpeechFeatureValue::category("consonant".into()).unwrap(),
                    )
                    .unwrap(),
                ),
                feature("laryngeal-voice", state),
                feature(
                    "aspiration",
                    FeatureSpecification::known(
                        SpeechFeatureValue::category("aspirated".into()).unwrap(),
                    )
                    .unwrap(),
                ),
            ])
            .unwrap(),
        )
        .unwrap();
        let original = SpeechPhone::new(
            BoundedSequence::new(),
            bundle,
            PhoneId::new("phone/t".into()).unwrap(),
            "tʰ".into(),
            SpeechSegmentStatus::Allophonic,
        )
        .unwrap();
        let result = prepare_greeting_phone_gestures(
            &event().encode().unwrap(),
            &membership().encode().unwrap(),
            &original.clone().encode().unwrap(),
            &selected().encode().unwrap(),
            &timing.clone().encode().unwrap(),
            policy(),
        );
        if index == 0 {
            let prepared = result.unwrap();
            assert_eq!(prepared.lowered().declared_phone(), &original);
            assert_eq!(
                prepared.lowered().declared_phone().features().get().len(),
                3
            );
            assert!(prepared
                .lowered()
                .executions()
                .iter()
                .any(|e| e.input_canonical()
                    == SpeechGreetingReviewedFeatureRequest::new(
                        original.features().get().clone(),
                        original.ipa().clone()
                    )
                    .unwrap()
                    .encode()
                    .unwrap()));
        } else {
            assert!(matches!(
                result,
                Err(SpeechGestureRefusal::UnsupportedFeatures)
            ));
        }
        assert!(matches!(
            prepare_declared_phone_gestures(
                &event().encode().unwrap(),
                &membership().encode().unwrap(),
                &original.encode().unwrap(),
                &selected().encode().unwrap(),
                &timing.clone().encode().unwrap()
            ),
            Err(SpeechGestureRefusal::UnsupportedFeatures)
        ));
    }
}
