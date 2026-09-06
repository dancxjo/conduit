use conduit_body::{
    Body, BodyBiographyError, BodyBiographyEvidence, BodyBiographyRecordKind, BodyMembership,
    ResidentForm,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};

fn born() -> BodyBiographyEvidence {
    let body = Body::born(
        SourceDocumentId::from("source/button"),
        CheckedFormId::from("checked/button"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Workbench".into(),
    )
    .unwrap()
}

#[test]
fn retains_exact_play_and_workload_change_before_explicit_lull() {
    use conduit_core::{bind_active_play, seal_plan, ExpandedFormId, FormIdentity};
    let mut history = born();
    let (body, wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    let plan = seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source/button"),
            checked_form_id: CheckedFormId::from("checked/button"),
            expanded_form_id: ExpandedFormId::from("expanded/button"),
        },
        vec![],
    );
    let play = bind_active_play(&plan.plan_id, &"host/local".into(), &"boot/local".into(), 1);
    let playing = wake
        .plan_ready(&plan, SignId::from("sign/plan"))
        .unwrap()
        .play_started(&play, SignId::from("sign/play"))
        .unwrap();
    history
        .append_wake(body.clone(), playing.clone(), 2)
        .unwrap();
    assert_eq!(
        history.wakes[0].plans[0].active_play_id,
        Some(play.active_play_id)
    );
    let changed = body
        .admit_form(
            ResidentForm::new(
                SourceDocumentId::from("source/clock"),
                CheckedFormId::from("checked/clock"),
            ),
            SignId::from("sign/add"),
        )
        .unwrap();
    history
        .append_body_workload_events(changed.clone(), &[(SignId::from("sign/add"), 5)])
        .unwrap();
    let notified = playing
        .workload_changed(&changed, SignId::from("sign/workload"))
        .unwrap();
    history
        .append_wake(changed.clone(), notified.clone(), 6)
        .unwrap();
    assert_eq!(history.wakes[0].plans[0].plan_id, plan.plan_id);
    let mut reordered = history.clone();
    reordered.records.swap(4, 5);
    reordered.records[4].sequence = 5;
    reordered.records[5].sequence = 6;
    assert!(reordered.validate().is_err());
    let lulled = notified.lull(SignId::from("sign/lull")).unwrap();
    let retained = changed
        .retain_after_lull(&lulled, SignId::from("sign/retained"))
        .unwrap();
    history.append_wake(retained, lulled, 7).unwrap();
    history.validate().unwrap();
}

#[test]
fn retains_closed_wakes_across_workload_changes_and_roundtrip() {
    let mut history = born();
    let identity = history.body_id.clone();
    let (awake, wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    history.append_wake(awake.clone(), wake.clone(), 2).unwrap();
    let lulled = wake.lull(SignId::from("sign/lull")).unwrap();
    // A Wake Lull alone does not assert that the Body retained it.
    history
        .append_wake(awake.clone(), lulled.clone(), 3)
        .unwrap();
    assert_eq!(history.body, awake);
    let retained = awake
        .retain_after_lull(&lulled, SignId::from("sign/retained"))
        .unwrap();
    history
        .append_wake(retained.clone(), lulled.clone(), 4)
        .unwrap();
    let changed = retained
        .admit_form(
            ResidentForm::new(
                SourceDocumentId::from("source/clock"),
                CheckedFormId::from("checked/clock"),
            ),
            SignId::from("sign/add"),
        )
        .unwrap();
    history
        .append_body_workload_events(changed.clone(), &[(SignId::from("sign/add"), 5)])
        .unwrap();
    let (next_body, next_wake) = changed.wake(2, SignId::from("sign/next-wake")).unwrap();
    history.append_wake(next_body, next_wake, 6).unwrap();
    assert_eq!(history.body_id, identity);
    assert_eq!(history.wakes[0], lulled);
    assert_eq!(history.wakes.len(), 2);
    let reopened: BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(&history).unwrap()).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened, history);
}

#[test]
fn rejects_rewritten_wake_stale_sequence_and_overflow_atomically() {
    let mut history = born();
    let (body, wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    history.append_wake(body.clone(), wake.clone(), 2).unwrap();
    let before = history.clone();
    let mut rewritten = wake.clone();
    rewritten.sign_ids[0] = SignId::from("sign/rewritten");
    rewritten.events[0] = conduit_body::WakeLifecycleEvent::Woke {
        sign_id: rewritten.sign_ids[0].clone(),
    };
    rewritten.validate().unwrap();
    assert_eq!(
        history.append_wake(body.clone(), rewritten, 3),
        Err(BodyBiographyError::InvalidEvidence)
    );
    let lulled = wake.lull(SignId::from("sign/lull")).unwrap();
    assert_eq!(
        history.append_wake(body.clone(), lulled.clone(), 2),
        Err(BodyBiographyError::InvalidSequence)
    );
    let retained = body
        .retain_after_lull(&lulled, SignId::from("sign/retained"))
        .unwrap();
    assert_eq!(
        history.append_wake(retained, lulled, u64::MAX),
        Err(BodyBiographyError::InvalidSequence)
    );
    assert_eq!(history, before);
}

#[test]
fn rejects_missing_duplicate_reordered_or_forged_lifecycle_records() {
    let mut history = born();
    let (body, wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    let wake = wake.lull(SignId::from("sign/lull")).unwrap();
    let body = body
        .retain_after_lull(&wake, SignId::from("sign/retained"))
        .unwrap();
    history.append_wake(body, wake, 2).unwrap();
    for mutation in 0..5 {
        let mut invalid = history.clone();
        match mutation {
            0 => {
                invalid.records.remove(2);
            }
            1 => {
                invalid.records[2].kind = invalid.records[1].kind.clone();
            }
            2 => {
                invalid.records[1].sign_id = SignId::from("sign/forged");
            }
            3 => {
                invalid.wakes.clear();
            }
            _ => {
                invalid.records[2].kind = BodyBiographyRecordKind::WakeEvent {
                    wake_id: invalid.wakes[0].wake_id.clone(),
                    event_index: 31,
                };
            }
        }
        assert!(invalid.validate().is_err(), "mutation {mutation}");
    }
}

#[test]
fn rejects_unrecorded_body_wake_and_wrong_initial_workload() {
    let mut history = born();
    let (body, mut wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    assert!(history
        .append_body_workload_events(body.clone(), &[])
        .is_err());
    wake.workload_revision = 99;
    wake.validate().unwrap();
    let before = history.clone();
    assert!(history.append_wake(body, wake, 2).is_err());
    assert_eq!(history, before);
}

#[test]
fn refuses_duplicate_wakes_and_noop_without_changing_history() {
    let mut history = born();
    let (body, wake) = history.body.wake(1, SignId::from("sign/woke")).unwrap();
    let wake = wake.fail(SignId::from("sign/failed")).unwrap();
    let body = body
        .retain_after_lull(&wake, SignId::from("sign/retained"))
        .unwrap();
    history.append_wake(body.clone(), wake.clone(), 2).unwrap();
    let before = history.clone();
    assert_eq!(
        history.append_wake(body.clone(), wake, 5),
        Err(BodyBiographyError::DuplicateEvidence)
    );
    let (reused_body, reused_wake) = body.wake(1, SignId::from("sign/reused")).unwrap();
    assert!(history.append_wake(reused_body, reused_wake, 5).is_err());
    assert_eq!(history, before);
    let mut duplicate = history.clone();
    duplicate.wakes.push(duplicate.wakes[0].clone());
    assert!(duplicate.validate().is_err());
    let mut overflow = history;
    overflow.wakes = vec![overflow.wakes[0].clone(); conduit_body::MAX_BODY_BIOGRAPHY_WAKES + 1];
    assert_eq!(
        overflow.validate(),
        Err(BodyBiographyError::CapacityExhausted)
    );
}
