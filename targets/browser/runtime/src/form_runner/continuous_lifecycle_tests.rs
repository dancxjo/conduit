use super::*;

const DRIVE_STEPS: usize = crate::installed_browser::BROWSER_ROUTE_SLOTS;

fn pending_index(
    session: &mut TourSession,
    matches: impl Fn(&engine::PendingHostEffect) -> bool,
    message: &str,
) -> usize {
    for _ in 0..DRIVE_STEPS {
        if let Some(index) = session.pending.iter().position(&matches) {
            return index;
        }
        let _ = session.poll_effect().unwrap();
    }
    panic!("{message}");
}

#[test]
fn keyboard_source_remains_eligible_after_the_first_event() {
    let source = include_str!("../../../../../forms/morse-network/main.conduit");
    let (mut session, first) =
        TourSession::prepare("browser/morse-proof", "boot/morse-proof", source, 61).unwrap();
    assert!(matches!(first, TourHostEffect::KeyEvent(_)));
    let play = session.active_play_id.as_str().to_owned();

    let key_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::KeyEvent),
        "morse source did not arm a keyboard request",
    );
    let key_request = session.pending[key_index].request;
    let key_placement = session.fragments[0].placements[usize::from(key_request.node.0)]
        .placement_id
        .as_str()
        .to_owned();
    let a = conduit_human::KeyEvent::new(
        0x04,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::NONE,
    )
    .unwrap()
    .encode();
    let _ = session
        .complete_effect(&play, &key_placement, key_request.request.0, Some(&a))
        .unwrap();
    let presentation_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_)),
        "morse key did not produce a manifestation",
    );
    let presentation_request = session.pending[presentation_index].request;
    let presentation_placement = session.fragments[0].placements
        [usize::from(presentation_request.node.0)]
    .placement_id
    .as_str()
    .to_owned();
    let _ = session
        .complete_effect(
            &play,
            &presentation_placement,
            presentation_request.request.0,
            None,
        )
        .unwrap();
    let next_key_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::KeyEvent),
        "morse source completed instead of re-arming keyboard input",
    );
    assert_eq!(session.pending[next_key_index].request.request.0, 1);
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn pointer_source_remains_eligible_after_one_observation() {
    let source = include_str!("../../../../../forms/pocket-theremin/main.conduit");
    let (mut session, first) = TourSession::prepare_with_profile(
        "browser/pointer-proof",
        "boot/pointer-proof",
        source,
        62,
        MorseRealization::Direct,
        crate::installed_browser::PresentationProfile::Quantity,
    )
    .unwrap();
    assert!(matches!(first, TourHostEffect::PointerEvent(_)));
    let play = session.active_play_id.as_str().to_owned();
    let pointer_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::PointerEvent),
        "pointer source did not arm an observation",
    );
    let pointer_request = session.pending[pointer_index].request;
    let pointer_placement = session.fragments[0].placements[usize::from(pointer_request.node.0)]
        .placement_id
        .as_str()
        .to_owned();
    let sample = conduit_semantic_catalog::normalized_pointer_value(
        conduit_semantic_catalog::NormalizedPointerSample {
            position_x: 250_000,
            position_y: 0,
            delta_x: 0,
            delta_y: 0,
            primary_pressed: false,
            coalesced: 2,
            dropped: 1,
            queue_capacity: 8,
            sequence: 0,
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let _ = session
        .complete_effect(
            &play,
            &pointer_placement,
            pointer_request.request.0,
            Some(&sample),
        )
        .unwrap();
    let output_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_)),
        "pointer source did not map a manifestation",
    );
    let output_request = session.pending[output_index].request;
    let output_placement = session.fragments[0].placements[usize::from(output_request.node.0)]
        .placement_id
        .as_str()
        .to_owned();
    let _ = session
        .complete_effect(&play, &output_placement, output_request.request.0, None)
        .unwrap();
    let next_pointer_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::PointerEvent),
        "pointer source completed after one observation",
    );
    assert_eq!(session.pending[next_pointer_index].request.request.0, 1);
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn ordinary_time_every_has_no_hidden_fixed_tick_lifetime() {
    let source = include_str!("../../../../../forms/firefly-choir/main.conduit");
    let (mut session, mut effect) =
        TourSession::prepare("browser/firefly-proof", "boot/firefly-proof", source, 63).unwrap();
    let play = session.active_play_id.as_str().to_owned();
    let mut pulse_count = 0_u32;
    for _ in 0..120 {
        match &effect {
            TourHostEffect::Timer(timer) => assert_eq!(timer.duration_millis, 240),
            TourHostEffect::Manifestation(value)
                if value.presentation_kind == conduit_semantic_catalog::PULSE_PRESENTATION_KIND =>
            {
                pulse_count += 1;
                if pulse_count >= 6 {
                    break;
                }
            }
            TourHostEffect::Manifestation(_) => {}
            _ => panic!("firefly requested an unrelated effect"),
        }
        match session.advance().unwrap() {
            TourProgress::Effect(next) => effect = *next,
            TourProgress::Receipt(receipt) => {
                panic!("time/every reached semantic completion unexpectedly: {receipt:?}")
            }
            TourProgress::Waiting { .. } | TourProgress::Cancellation { .. } => {}
        }
    }
    assert!(pulse_count >= 6);
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn reactive_transforms_accept_later_values_on_open_flows() {
    let source = include_str!("../../../../../forms/memory-lantern/main.conduit");
    let (mut session, first) =
        TourSession::prepare("browser/memory-proof", "boot/memory-proof", source, 64).unwrap();
    assert!(matches!(first, TourHostEffect::KeyEvent(_)));
    let play = session.active_play_id.as_str().to_owned();
    let mut outputs = Vec::new();
    for usage in [0x04, 0x05] {
        let key_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::KeyEvent),
            "memory source did not request the next key",
        );
        let request = session.pending[key_index].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let encoded = conduit_human::KeyEvent::new(
            usage,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap()
        .encode();
        let _ = session
            .complete_effect(&play, &placement, request.request.0, Some(&encoded))
            .unwrap();
        let output_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_)),
            "memory source did not present updated state",
        );
        let TourHostEffect::Manifestation(value) =
            session.project_pending_effect(output_index).unwrap()
        else {
            unreachable!()
        };
        outputs.push(value.text.unwrap());
        let output_request = session.pending[output_index].request;
        let output_placement = session.fragments[0].placements[usize::from(output_request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let _ = session
            .complete_effect(&play, &output_placement, output_request.request.0, None)
            .unwrap();
    }
    assert_eq!(outputs, ["a", "ab"]);
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn button_source_lifetime_is_not_limited_by_one_attempt_configuration() {
    let source = include_str!("../../../../../forms/button-across-room/main.conduit");
    let (mut session, first) =
        TourSession::prepare("browser/button-proof", "boot/button-proof", source, 65).unwrap();
    assert!(matches!(first, TourHostEffect::ButtonTransition(_)));
    let play = session.active_play_id.as_str().to_owned();

    for sequence in 0..8_u64 {
        let button_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::ButtonTransition),
            "button source did not remain armed for another transition",
        );
        let request = session.pending[button_index].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let pressed = sequence.is_multiple_of(2);
        let transition =
            conduit_semantic_catalog::button_transition_value("button/primary", pressed, sequence)
                .unwrap()
                .canonical_bytes()
                .unwrap();
        let _ = session
            .complete_effect(&play, &placement, request.request.0, Some(&transition))
            .unwrap();

        let output_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_)),
            "button transition did not emit a manifestation",
        );
        let output_request = session.pending[output_index].request;
        let output_placement = session.fragments[0].placements[usize::from(output_request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let _ = session
            .complete_effect(&play, &output_placement, output_request.request.0, None)
            .unwrap();
    }

    let next_button_index = pending_index(
        &mut session,
        |pending| matches!(pending.effect, engine::BrowserHostEffect::ButtonTransition),
        "button source completed after finite transitions instead of staying open",
    );
    assert_eq!(session.pending[next_button_index].request.request.0, 8);
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn cancelling_while_a_timer_is_pending_stays_distinct_from_semantic_completion() {
    let source = include_str!("../../../../../forms/firefly-choir/main.conduit");
    let (session, first) = TourSession::prepare(
        "browser/timer-cancel-proof",
        "boot/timer-cancel-proof",
        source,
        66,
    )
    .unwrap();
    let TourHostEffect::Timer(timer) = first else {
        panic!("firefly should begin with a standing timer");
    };
    let play = timer.active_play_id.clone();
    let receipt = session.cancel().unwrap();
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.active_play_id, play);
    assert_eq!(receipt.timer_completions, 0);
    assert_eq!(receipt.manifestation_completions, 0);
}

#[test]
fn downstream_pending_capacity_failure_remains_distinct_from_semantic_completion() {
    let source = "form concurrent {\n button: input/button\n state: input/button-indicator-state\n indicator: presentation/indicator-state\n clock: time/every(freq = 100ms)\n count: state/count(start = 0)\n show: presentation/count\n button > state > indicator\n clock.tick > count.bump\n count.value > show.value\n}\n";
    let (mut session, _) =
        TourSession::prepare("browser/capacity-proof", "boot/capacity-proof", source, 67).unwrap();
    session.pending = Vec::with_capacity(0);
    let error = session.poll_effect().unwrap_err();
    assert!(error.contains("browser pending effect capacity exhausted"));
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn explicit_transactional_form_still_reaches_semantic_completion() {
    let source = "form text-chain {\n complete\n source: text/literal(\"hello\")\n upper: text/upper\n result: presentation/text\n source > upper > result\n}\n";
    let (session, first) = TourSession::prepare(
        "browser/completion-proof",
        "boot/completion-proof",
        source,
        68,
    )
    .unwrap();
    assert!(matches!(first, TourHostEffect::Manifestation(_)));
    assert_eq!(session.complete().unwrap().disposition, "completed");
}

#[test]
fn repeated_inputs_do_not_restart_plan_or_play() {
    let source = include_str!("../../../../../forms/morse-network/main.conduit");
    let (mut session, first) =
        TourSession::prepare("browser/restart-proof", "boot/restart-proof", source, 69).unwrap();
    assert!(matches!(first, TourHostEffect::KeyEvent(_)));
    let play = session.active_play_id.as_str().to_owned();
    let mut identities = None;
    for usage in [0x16, 0x12] {
        let key_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::KeyEvent),
            "morse source did not keep keyboard input armed",
        );
        let request = session.pending[key_index].request;
        let placement = session.fragments[0].placements[usize::from(request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let encoded = conduit_human::KeyEvent::new(
            usage,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap()
        .encode();
        let _ = session
            .complete_effect(&play, &placement, request.request.0, Some(&encoded))
            .unwrap();

        let output_index = pending_index(
            &mut session,
            |pending| matches!(pending.effect, engine::BrowserHostEffect::Manifestation(_)),
            "morse input did not yield a manifestation",
        );
        let TourHostEffect::Manifestation(effect) =
            session.project_pending_effect(output_index).unwrap()
        else {
            unreachable!()
        };
        if let Some((checked_form, plan, active_play)) = &identities {
            assert_eq!(&effect.checked_form_id, checked_form);
            assert_eq!(&effect.plan_id, plan);
            assert_eq!(&effect.active_play_id, active_play);
        } else {
            identities = Some((
                effect.checked_form_id.clone(),
                effect.plan_id.clone(),
                effect.active_play_id.clone(),
            ));
        }
        let output_request = session.pending[output_index].request;
        let output_placement = session.fragments[0].placements[usize::from(output_request.node.0)]
            .placement_id
            .as_str()
            .to_owned();
        let _ = session
            .complete_effect(&play, &output_placement, output_request.request.0, None)
            .unwrap();
    }
    assert_eq!(session.active_play_id.as_str(), play);
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}
