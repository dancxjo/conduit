use conduit_body::*;
use conduit_core::{seal_plan, FormIdentity, SignId};

fn born(sequence: u64) -> BodyBiographyEvidence {
    let body = Body::born(
        "source/cue".into(),
        "checked/cue".into(),
        sequence,
        sign("birth", sequence),
    )
    .unwrap();
    BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Juniper".into(),
    )
    .unwrap()
}
fn sign(name: &str, sequence: u64) -> SignId {
    format!("sign/{name}/{sequence}").into()
}
fn next(history: &BodyBiographyEvidence) -> u64 {
    history.records.last().unwrap().sequence + 1
}
fn start(
    history: &mut BodyBiographyEvidence,
    body: Body,
    wake: Wake,
    sequence: u64,
) -> (BodyPlan, BodyPlayIdentity) {
    let plan = BodyPlan::seal(
        &wake,
        wake.workset
            .forms()
            .iter()
            .map(|form| BodyFormPlan {
                form: form.clone(),
                plan: seal_plan(
                    FormIdentity {
                        source_document_id: form.source_document_id.clone(),
                        checked_form_id: form.checked_form_id.clone(),
                        expanded_form_id: format!("expanded/{}", form.checked_form_id.as_str())
                            .into(),
                    },
                    vec![],
                ),
            })
            .collect(),
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, sequence);
    let started = wake
        .body_plan_ready(&plan, sign("plan", sequence))
        .unwrap()
        .body_play_started(&plan, &play, sign("play", sequence))
        .unwrap();
    history.append_wake(body, started, next(history)).unwrap();
    (plan, play)
}
fn wake(history: &mut BodyBiographyEvidence, sequence: u64) -> (BodyPlan, BodyPlayIdentity) {
    let (body, wake) = history.body.wake(sequence, sign("wake", sequence)).unwrap();
    start(history, body, wake, sequence)
}
fn lull(history: &mut BodyBiographyEvidence, sequence: u64) {
    let wake = history
        .wakes
        .last()
        .unwrap()
        .lull(sign("lull", sequence))
        .unwrap();
    let body = history
        .body
        .retain_after_lull(&wake, sign("retained", sequence))
        .unwrap();
    history.append_wake(body, wake, next(history)).unwrap();
}

#[test]
fn first_body_wake_and_each_wake_are_distinct_after_durable_roundtrip_and_rebirth() {
    let mut history = born(1);
    let body_id = history.body_id.clone();
    let (plan, play) = wake(&mut history, 1);
    let first = history.startup_for_play(&plan, &play).unwrap();
    assert!(first.eligible(StartupScope::Body));
    assert!(first.eligible(StartupScope::Wake));
    assert_eq!(first.wake_sign_id, sign("wake", 1));
    assert_eq!(first.play_start_sign_id, sign("play", 1));
    lull(&mut history, 1);
    let bytes = serde_json::to_vec(&history).unwrap();
    let mut restored: BodyBiographyEvidence = serde_json::from_slice(&bytes).unwrap();
    let (plan, play) = wake(&mut restored, 2);
    let later = restored.startup_for_play(&plan, &play).unwrap();
    assert_eq!(later.body_id, body_id);
    assert!(later.eligible(StartupScope::Wake));
    assert!(!later.eligible(StartupScope::Body));
    let mut reborn = born(9);
    let (plan, play) = wake(&mut reborn, 1);
    assert_ne!(reborn.body_id, body_id);
    assert!(reborn
        .startup_for_play(&plan, &play)
        .unwrap()
        .eligible(StartupScope::Body));
}

#[test]
fn replacement_play_in_the_same_wake_does_not_repeat_either_startup_scope() {
    let mut history = born(1);
    let (old_plan, old_play) = wake(&mut history, 1);
    let body = history
        .body
        .admit_form(
            ResidentForm::new("source/other".into(), "checked/other".into()),
            sign("add", 1),
        )
        .unwrap();
    let at = next(&history);
    history
        .append_body_workload_events(body.clone(), &[(sign("add", 1), at)])
        .unwrap();
    let changed = history.wakes[0]
        .workload_changed(&body, sign("workload", 1))
        .unwrap();
    history
        .append_wake(body.clone(), changed.clone(), next(&history))
        .unwrap();
    let (new_plan, new_play) = start(&mut history, body, changed, 2);
    assert_ne!(old_plan.plan_id, new_plan.plan_id);
    assert_eq!(old_play.wake_id, new_play.wake_id);
    let replacement = history.startup_for_play(&new_plan, &new_play).unwrap();
    assert!(!replacement.eligible(StartupScope::Wake));
    assert!(!replacement.eligible(StartupScope::Body));
    assert!(history.startup_for_play(&old_plan, &old_play).is_err());
}

#[test]
fn absent_or_altered_history_and_unstarted_or_old_plays_cannot_supply_startup() {
    let mut history = born(1);
    let (plan, play) = wake(&mut history, 1);
    let forged_play = BodyPlayIdentity::bind(&plan, 99);
    assert_eq!(
        history.startup_for_play(&plan, &forged_play),
        Err(BodyStartupRefusal::StalePlay)
    );
    let mut forged_plan = plan.clone();
    forged_plan.plan_id = "invented".into();
    assert_eq!(
        history.startup_for_play(&forged_plan, &play),
        Err(BodyStartupRefusal::StalePlan)
    );
    let mut missing = history.clone();
    missing.records.remove(1);
    assert_eq!(
        missing.startup_for_play(&plan, &play),
        Err(BodyStartupRefusal::InvalidEvidence)
    );
    lull(&mut history, 1);
    assert_eq!(
        history.startup_for_play(&plan, &play),
        Err(BodyStartupRefusal::NotAwake)
    );
}
