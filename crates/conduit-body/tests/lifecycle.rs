use conduit_body::{
    Body, BodyLifecycleError, BodyLifecycleEvent, BodyState, WakeLifecycle, WakeLifecycleEvent,
    WakePlanState, MAX_WAKE_CLUES,
};
use conduit_core::{bind_active_play, CheckedFormId, ClueId, Plan, PlanId, SourceDocumentId};

fn plan(identity: &str) -> Plan {
    Plan {
        source_document_id: SourceDocumentId::from("source-a"),
        checked_form_id: CheckedFormId::from("checked-a"),
        expanded_form_id: "expanded".into(),
        plan_id: PlanId::from(identity),
        fragments: vec![],
    }
}
fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source-a"),
        CheckedFormId::from("checked-a"),
        4,
        ClueId::from("bornd"),
    )
    .unwrap()
}

#[test]
fn body_survives_lull_and_one_wake_survives_replan() {
    let body = body();
    let body_id = body.body_id.clone();
    let (awake, wake) = body.wake(9, ClueId::from("woke")).unwrap();
    let plan_a = plan("plan-a");
    let wake = wake.plan_ready(&plan_a, ClueId::from("planned-a")).unwrap();
    let play_a = bind_active_play(&plan_a.plan_id, &"host-a".into(), &"boot-a".into(), 1);
    let wake = wake
        .play_started(&play_a, ClueId::from("playing-a"))
        .unwrap();
    let wake = wake
        .became_unsatisfied(&plan_a.plan_id, ClueId::from("unsatisfied-a"))
        .unwrap();
    let plan_b = plan("plan-b");
    let wake = wake.plan_ready(&plan_b, ClueId::from("planned-b")).unwrap();
    let play_b = bind_active_play(&plan_b.plan_id, &"host-b".into(), &"boot-b".into(), 2);
    let wake = wake
        .play_started(&play_b, ClueId::from("playing-b"))
        .unwrap();
    assert_eq!(wake.plans[0].state, WakePlanState::Superseded);
    assert_ne!(wake.plans[0].active_play_id, wake.plans[1].active_play_id);
    let lulled_wake = wake.lull(ClueId::from("lulled")).unwrap();
    let retained = awake
        .retain_after_lull(&lulled_wake, ClueId::from("retained"))
        .unwrap();
    assert_eq!(retained.body_id, body_id);
    assert_eq!(retained.state, BodyState::Lulled);
    let (_, next_wake) = retained.wake(10, ClueId::from("rewoke")).unwrap();
    assert_ne!(next_wake.wake_id, lulled_wake.wake_id);
}

#[test]
fn stale_inputs_and_duplicate_clue_fail_closed() {
    let (awake, wake) = body().wake(1, ClueId::from("woke")).unwrap();
    assert_eq!(
        wake.lull(ClueId::from("woke")),
        Err(BodyLifecycleError::DuplicateClue)
    );
    let stale = Plan {
        source_document_id: SourceDocumentId::from("other"),
        ..plan("stale")
    };
    assert_eq!(
        wake.plan_ready(&stale, ClueId::from("stale")),
        Err(BodyLifecycleError::StalePlan)
    );
    assert_eq!(
        awake.retain_after_lull(&wake, ClueId::from("too-soon")),
        Err(BodyLifecycleError::MismatchedWake)
    );
    assert_eq!(wake.lifecycle, WakeLifecycle::AwaitingPlan);
}

#[test]
fn typed_events_are_the_exact_clue_history_and_tampering_fails_closed() {
    let (awake, wake) = body().wake(1, ClueId::from("woke")).unwrap();
    assert!(matches!(
        awake.events.last(),
        Some(BodyLifecycleEvent::Woke { wake_id, clue_id })
            if wake_id == &wake.wake_id && clue_id.as_str() == "woke"
    ));
    let exact = plan("exact");
    let planned = wake.plan_ready(&exact, ClueId::from("planned")).unwrap();
    assert!(matches!(
        planned.events.last(),
        Some(WakeLifecycleEvent::PlanReady { plan_id, clue_id })
            if plan_id == &exact.plan_id && clue_id.as_str() == "planned"
    ));

    let mut drifted = planned;
    if let WakeLifecycleEvent::PlanReady { plan_id, .. } = &mut drifted.events[1] {
        *plan_id = PlanId::from("drifted");
    }
    assert_eq!(
        drifted.validate(),
        Err(BodyLifecycleError::InvalidTransition)
    );
}

#[test]
fn wake_clue_history_is_finite() {
    let (_, wake) = body().wake(1, ClueId::from("woke")).unwrap();
    let exact = plan("bounded");
    let waiting = wake.plan_ready(&exact, ClueId::from("planned")).unwrap();
    let play = bind_active_play(&exact.plan_id, &"host".into(), &"boot".into(), 1);
    let mut playing = waiting
        .play_started(&play, ClueId::from("playing"))
        .unwrap();
    for index in 3..MAX_WAKE_CLUES {
        playing = playing
            .same_plan_observed(&exact.plan_id, ClueId::from(format!("observation-{index}")))
            .unwrap();
    }
    assert_eq!(
        playing.same_plan_observed(&exact.plan_id, ClueId::from("overflow")),
        Err(BodyLifecycleError::ClueCapacityExhausted)
    );
}
