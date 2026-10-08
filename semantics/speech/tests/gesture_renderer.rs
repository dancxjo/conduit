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

fn grid(rate: u64) -> conduit_audio::AudioSampleRateBasis {
    conduit_audio::AudioSampleRateBasis::new(
        acoustic::anchor(),
        conduit_audio::AudioFrameQuantization::Floor,
        rate,
    )
    .unwrap()
}
#[test]
fn actual_renderer_obeys_original_explicit_windows_at_three_rates() {
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    let cycle = conduit_audio::AudioCycleDuration::new(200, 1)
        .unwrap()
        .encode()
        .unwrap();
    for rate in [8000, 16000, 48000] {
        let original = prepared_fixture("tʰ", &timing).unwrap();
        let plan =
            prepare_speech_gesture_renderer(&original, &grid(rate).encode().unwrap(), &cycle)
                .unwrap();
        assert!(core::ptr::eq(plan.original(), &original));
        assert_eq!(plan.original_cycle_canonical(), cycle);
        assert_eq!(
            *plan.cycle_projection().result().raw().whole_frames(),
            rate / 200
        );
        assert_eq!(plan.frame_range(), 0..i32::try_from(rate / 10).unwrap());
        assert_eq!(
            *plan.windows().closure().end(),
            i32::try_from(rate * 3 / 50).unwrap()
        );
        assert_eq!(
            *plan.windows().release().start(),
            i32::try_from(rate * 3 / 50).unwrap()
        );
        assert_eq!(
            *plan.windows().release().end(),
            i32::try_from(rate * 4 / 50).unwrap()
        );
        assert_eq!(
            *plan.windows().aspiration().start(),
            i32::try_from(rate * 4 / 50).unwrap()
        );
        assert_eq!(
            *plan.windows().aspiration().end(),
            i32::try_from(rate / 10).unwrap()
        );
        // Every formant is evaluated by the real Audio step capability at an
        // actual sample-grid query, including full-sized Hz and denominator.
        for trajectory in plan.trajectories() {
            let g = trajectory.original();
            if !matches!(
                g.channel(),
                SpeechGestureChannel::FormantCenter | SpeechGestureChannel::FormantBandwidth
            ) {
                continue;
            }
            let prepared = conduit_audio::PreparedAudioQuantityTrajectory::new(
                trajectory.admitted_canonical(),
            )
            .unwrap();
            let q = conduit_audio::AudioTrajectoryQuery::new(
                acoustic::anchor(),
                conduit_audio::AudioExactTimeOffset::new(rate, 1).unwrap(),
            )
            .unwrap();
            let evaluated = prepared.evaluate(&q.encode().unwrap()).unwrap();
            assert_eq!(evaluated.result(), g.quantity());
        }
        let mut cursor = plan.cursor();
        let mut count = 0;
        let mut audible = 0;
        while let Some(frame) = plan.next(&mut cursor).unwrap() {
            if frame.frame() == 0 || frame.frame() == i32::try_from(rate * 3 / 50).unwrap() {
                replay_frame(&frame);
            }
            let n = frame.frame() as u64;
            assert_eq!(*frame.gates().closure(), n < rate * 3 / 50);
            assert_eq!(
                *frame.gates().release(),
                n >= rate * 3 / 50 && n < rate * 4 / 50
            );
            assert_eq!(*frame.gates().aspiration(), n >= rate * 4 / 50);
            assert!(!*frame.gates().frication());
            assert!(!*frame.gates().voiced());
            if *frame.gates().closure() {
                assert_eq!(frame.sample(), 0)
            } else if frame.sample() != 0 {
                audible += 1
            }
            assert_eq!(frame.executions().len(), 1);
            count += 1;
        }
        assert_eq!(count, rate / 10);
        assert!(audible > 0);
        println!("rate={rate} frames={count} actual-nonzero-after-closure={audible}");
    }
}
#[test]
fn contextual_role_contrasts_and_foreign_or_unsupported_profiles_refuse() {
    let timing = t(time(0, 10), time(1, 10), time(0, 10), time(1, 10));
    let cycle = conduit_audio::AudioCycleDuration::new(200, 1)
        .unwrap()
        .encode()
        .unwrap();
    let basis = grid(8000).encode().unwrap();
    let mut waves = Vec::new();
    for ipa in ["i", "a", "u", "t", "d", "tʰ", "t͡ʃ"] {
        let original = prepared_fixture(ipa, &timing).unwrap();
        let plan = prepare_speech_gesture_renderer(&original, &basis, &cycle).unwrap();
        let mut cursor = plan.cursor();
        let mut samples = Vec::new();
        while let Some(frame) = plan.next(&mut cursor).unwrap() {
            samples.push(frame.sample());
        }
        assert!(samples.iter().any(|s| *s != 0));
        waves.push(samples);
    }
    if let Some(path) = std::env::var_os("CONDUIT_GESTURE_PROOF_WAV") {
        let samples = waves
            .iter()
            .flatten()
            .map(|s| i16::try_from(*s).unwrap())
            .collect::<Vec<_>>();
        let bytes = u32::try_from(samples.len() * 2).unwrap();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend_from_slice(&16000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for s in samples {
            wav.extend_from_slice(&s.to_le_bytes())
        }
        std::fs::write(path, wav).unwrap();
    }
    for (a, b) in [(0, 1), (1, 2), (3, 4), (3, 5), (5, 6)] {
        assert_ne!(waves[a], waves[b]);
    }
    let first = prepared_fixture("i", &timing).unwrap();
    let second = prepared_fixture("a", &timing).unwrap();
    let plan = prepare_speech_gesture_renderer(&first, &basis, &cycle).unwrap();
    let other = prepare_speech_gesture_renderer(&second, &basis, &cycle).unwrap();
    assert!(matches!(
        other.next(&mut plan.cursor()),
        Err(SpeechGestureRenderRefusal::ForeignBasis)
    ));
    let fractional = conduit_audio::AudioCycleDuration::new(201, 1)
        .unwrap()
        .encode()
        .unwrap();
    assert!(matches!(
        prepare_speech_gesture_renderer(&first, &basis, &fractional),
        Err(SpeechGestureRenderRefusal::Admission(_))
    ));
    assert!(
        prepare_speech_gesture_renderer(&first, &grid(22050).encode().unwrap(), &cycle).is_err()
    );
    let invalid = SpeechGestureFrameWindow::new(10, 11);
    assert!(invalid.is_err());
    let empty = SpeechGestureFrameWindow::new(0, 0).unwrap();
    let overlap = SpeechGestureFrameWindow::new(20, 10).unwrap();
    assert!(SpeechGestureFrameWindows::new(
        overlap.clone(),
        empty.clone(),
        800,
        empty.clone(),
        overlap,
        empty
    )
    .is_err());
}

fn replay_frame(frame: &SpeechGestureRenderedFrame) {
    use conduit_speech::gesture_renderer::speech_gesture_dsp_programs as graph;
    if frame.frame() == 0 {
        let input = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            graph::PROGRAMS[graph::INPUTS[0]],
        )
        .unwrap();
        let output = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            graph::PROGRAMS[graph::RESULT],
        )
        .unwrap();
        println!(
            "actual DSP input Type {} value {}; output Type {} value {}; window Type {}",
            input.input_type.canonical_bytes().unwrap().len(),
            frame.dsp_input_canonical().len(),
            output.output_type.canonical_bytes().unwrap().len(),
            frame.dsp_output_canonical().len(),
            SpeechGestureFrameWindows::semantic_type()
                .unwrap()
                .canonical_bytes()
                .unwrap()
                .len()
        );
    }
    let mut values = vec![None::<Vec<u8>>; graph::PROGRAMS.len()];
    for _ in 0..graph::PROGRAMS.len() {
        let mut progress = false;
        for (index, hex) in graph::PROGRAMS.iter().enumerate() {
            if values[index].is_some() {
                continue;
            }
            let input = if graph::INPUTS.contains(&index) {
                Some(frame.dsp_input_canonical())
            } else {
                graph::CONNECTIONS
                    .iter()
                    .find(|(_, sink)| *sink == index)
                    .and_then(|(source, _)| values[*source].as_deref())
            };
            if let Some(input) = input {
                values[index] = Some(
                    conduit_plot::PortableExpressionProgram::from_canonical_hex(hex)
                        .unwrap()
                        .evaluate(input)
                        .unwrap(),
                );
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    assert_eq!(
        values[graph::RESULT].as_deref(),
        Some(frame.dsp_output_canonical())
    );
}
