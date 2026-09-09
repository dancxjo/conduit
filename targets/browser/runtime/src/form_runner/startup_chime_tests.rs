//! Canonical sound-only Forms share the kernel and preserve optional audio outcomes.
use super::{
    body_start::{prepare, tests::request_from_sources},
    *,
};

#[test]
fn canonical_chime_requests_one_bounded_audio_effect_then_idles_without_graphics() {
    let request = request_from_sources(&[include_str!(
        "../../../../../forms/startup-chime/main.conduit"
    )]);
    let (mut session, started) = prepare(request).unwrap();
    let TourProgress::Effect(effect) = started.progress else {
        panic!("missing cue effect")
    };
    let TourHostEffect::AudioCue(effect) = *effect else {
        panic!("cue asked for graphics or input")
    };
    assert_eq!(effect.frames, conduit_synth::STARTUP_CHIME_FRAMES);
    assert_eq!(effect.sample_rate, 48_000);
    assert_eq!(effect.channels, 1);
    assert_eq!(effect.score_id, conduit_synth::STARTUP_CHIME_SCORE_ID);
    assert_eq!(effect.active_play_id, started.play.active_play_id.as_str());
    let progress = session
        .complete_effect(
            &effect.active_play_id,
            &effect.placement_id,
            effect.request_sequence,
            None,
        )
        .unwrap();
    assert!(matches!(
        progress,
        TourProgress::Waiting {
            disposition: "quiescent_awaiting_input",
            pending_effects: 0,
            ..
        }
    ));
    assert!(session
        .complete_effect(
            &effect.active_play_id,
            &effect.placement_id,
            effect.request_sequence,
            None
        )
        .is_err());
    assert!(matches!(
        session.poll_effect().unwrap(),
        TourProgress::Waiting {
            pending_effects: 0,
            ..
        }
    ));
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn denied_and_unavailable_audio_remain_visible_while_keyboard_forms_continue() {
    for denied in [true, false] {
        let request = request_from_sources(&[
            include_str!("../../../../../forms/startup-chime/main.conduit"),
            include_str!("../../../../../forms/memory-lantern/main.conduit"),
        ]);
        let (mut session, started) = prepare(request).unwrap();
        let mut progress = started.progress;
        let mut cue = None;
        let mut keyboard = None;
        for _ in 0..8 {
            match progress {
                TourProgress::Effect(effect) => match *effect {
                    TourHostEffect::AudioCue(effect) => cue = Some(effect),
                    TourHostEffect::KeyEvent(effect) => keyboard = Some(effect),
                    _ => panic!("unexpected startup effect"),
                },
                TourProgress::Waiting { .. } => break,
                _ => panic!("Body terminated before input"),
            }
            progress = session.poll_effect().unwrap();
        }
        let cue = cue.unwrap();
        let keyboard = keyboard.unwrap();
        assert!(session
            .refuse_effect("stale", &cue.placement_id, cue.request_sequence, denied, 1)
            .is_err());
        assert_eq!(session.pending.len(), 2);
        assert!(matches!(
            session
                .refuse_effect(
                    &cue.active_play_id,
                    &cue.placement_id,
                    cue.request_sequence,
                    denied,
                    1
                )
                .unwrap(),
            TourProgress::Waiting {
                pending_effects: 1,
                ..
            }
        ));
        let key = conduit_human::KeyEvent::new(
            0x04,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap()
        .encode();
        let mut progress = session
            .complete_effect(
                &keyboard.active_play_id,
                &keyboard.placement_id,
                keyboard.request_sequence,
                Some(&key),
            )
            .unwrap();
        let mut visible = false;
        for _ in 0..8 {
            match progress {
                TourProgress::Effect(effect) => match *effect {
                    TourHostEffect::Manifestation(effect) => {
                        assert_eq!(effect.text.as_deref(), Some("a"));
                        visible = true;
                        break;
                    }
                    TourHostEffect::KeyEvent(_) => {}
                    _ => panic!("unexpected effect after audio outcome"),
                },
                _ => panic!("keyboard stopped after audio outcome"),
            }
            progress = session.poll_effect().unwrap();
        }
        assert!(visible);
        let receipt = session.cancel().unwrap();
        assert_eq!(receipt.disposition, "cancelled");
        let evidence = serde_json::to_value(&receipt).unwrap();
        let records = evidence["kernel_signs"]["host_completions"]["records"]
            .as_array()
            .unwrap();
        assert!(records.iter().any(|record| record["disposition"]
            == if denied { "denied" } else { "failed" }
            && record["failure_code"]
                == if denied {
                    "host_operation_denied"
                } else {
                    "host_operation_failed"
                }
            && record["failure_detail"] == 1));
    }
}

#[test]
fn cue_preparation_reuses_the_canonical_dsp_and_never_changes_its_buffer() {
    let pointer = super::audio::conduit_browser_startup_chime_prepare();
    assert_eq!(
        pointer,
        super::audio::conduit_browser_startup_chime_prepare()
    );
    assert_eq!(super::audio::conduit_browser_startup_chime_frames(), 57_600);
    // This is the buffer from the pure preparation export, not an audio device.
    let prepared = unsafe { core::slice::from_raw_parts(pointer as *const i16, 57_600) };
    let mut renderer = conduit_synth::StartupChime::new();
    for expected in prepared.chunks(256) {
        let mut block = [0; 256];
        let count = renderer.render(&mut block[..expected.len()]).unwrap();
        assert_eq!(count, expected.len());
        assert_eq!(&block[..count], expected);
    }
    assert!(prepared.iter().any(|sample| *sample != 0));
    assert!(renderer.is_finished());
}

#[test]
fn canonical_first_wake_chime_does_not_repeat_after_retained_wake() {
    use conduit_body::{BodyBiographyEvidence, BodyPlan};
    let source = include_str!("../../../../../forms/first-wake-chime/main.conduit");
    let request = request_from_sources(&[source]);
    let mut history = request.body_evidence.clone().unwrap();
    let next = |history: &BodyBiographyEvidence| history.records.last().unwrap().sequence + 1;
    let (mut session, started) = prepare(request).unwrap();
    assert!(
        matches!(started.progress, TourProgress::Effect(ref effect) if matches!(**effect, TourHostEffect::AudioCue(_)))
    );
    history
        .append_wake(
            history.body.clone(),
            started.wake_at_start.clone(),
            next(&history),
        )
        .unwrap();
    assert!(matches!(
        session.advance().unwrap(),
        TourProgress::Waiting {
            pending_effects: 0,
            ..
        }
    ));
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
    let wake = started.wake_at_start.lull("sign/lull".into()).unwrap();
    let body = history
        .body
        .retain_after_lull(&wake, "sign/retained".into())
        .unwrap();
    history.append_wake(body, wake, next(&history)).unwrap();
    let mut restored: BodyBiographyEvidence =
        serde_json::from_slice(&serde_json::to_vec(&history).unwrap()).unwrap();
    let (body, wake) = restored.body.wake(2, "sign/second-wake".into()).unwrap();
    restored
        .append_wake(body, wake.clone(), next(&restored))
        .unwrap();
    let mut later = request_from_sources(&[source]);
    later.plan = BodyPlan::seal(&wake, later.plan.forms).unwrap();
    later.wake = wake;
    later.body_evidence = Some(restored);
    later.play_sequence += 1;
    let (session, started) = prepare(later).unwrap();
    assert!(matches!(
        started.progress,
        TourProgress::Waiting {
            disposition: "quiescent_awaiting_input",
            pending_effects: 0,
            ..
        }
    ));
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn live_inspection_exposes_kernel_gaps_and_a_bounded_host_completion_window() {
    let source = include_str!("../../../../../forms/button-across-room/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("host/gaps", "boot/gaps", source, 1).unwrap();
    let mut transitions = 0_u64;
    for _ in 0..100 {
        let (play, placement, request, output) = match effect {
            TourHostEffect::ButtonTransition(value) => {
                let bytes = conduit_semantic_catalog::button_transition_value(
                    "button/primary",
                    transitions.is_multiple_of(2),
                    transitions,
                )
                .unwrap()
                .canonical_bytes()
                .unwrap();
                transitions += 1;
                (
                    value.active_play_id,
                    value.placement_id,
                    value.request_sequence,
                    Some(bytes),
                )
            }
            TourHostEffect::Manifestation(value) => (
                value.active_play_id,
                value.placement_id,
                value.observation_sequence,
                None,
            ),
            _ => panic!("unexpected button effect"),
        };
        let progress = session
            .complete_effect(&play, &placement, request, output.as_deref())
            .unwrap();
        effect = match progress {
            TourProgress::Effect(effect) => *effect,
            _ => panic!("button stopped before its observation window filled"),
        };
    }
    let observed = serde_json::to_value(session.kernel_signs()).unwrap();
    assert!(observed["retention_gap"]["entries"].as_u64().unwrap() > 0);
    assert_eq!(observed["host_completions"]["omitted"], 36);
    assert_eq!(
        observed["host_completions"]["records"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}
