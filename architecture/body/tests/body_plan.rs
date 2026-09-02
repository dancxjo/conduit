use conduit_body::{
    Body, BodyFormPlan, BodyLifecycleError, BodyPlan, BodyPlanError, BodyPlayIdentity,
    ResidentForm, WakeLifecycle,
};
use conduit_core::{
    seal_plan, CheckedFormId, ExpandedFormId, FormIdentity, Plan, SignId, SourceDocumentId,
};

fn resident(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

fn plan(form: &ResidentForm, expansion: &str) -> Plan {
    seal_plan(
        FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: ExpandedFormId::from(format!("expanded/{expansion}")),
        },
        vec![],
    )
}

fn two_form_wake() -> conduit_body::Wake {
    let seed = resident("dashboard");
    let body = Body::born(
        seed.source_document_id,
        seed.checked_form_id,
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .admit_form(resident("service"), SignId::from("sign/admit-service"))
    .unwrap();
    body.wake(1, SignId::from("sign/woke")).unwrap().1
}

#[test]
fn one_body_plan_and_one_play_cover_two_exact_forms() {
    let wake = two_form_wake();
    let dashboard = resident("dashboard");
    let service = resident("service");
    let body_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: service.clone(),
                plan: plan(&service, "service"),
            },
            BodyFormPlan {
                form: dashboard.clone(),
                plan: plan(&dashboard, "dashboard"),
            },
        ],
    )
    .unwrap();
    assert_eq!(body_plan.forms[0].form, dashboard);
    assert_eq!(body_plan.forms[1].form, service);

    let waiting = wake
        .body_plan_ready(&body_plan, SignId::from("sign/planned"))
        .unwrap();
    let play = BodyPlayIdentity::bind(&body_plan, 1);
    let playing = waiting
        .body_play_started(&body_plan, &play, SignId::from("sign/playing"))
        .unwrap();
    assert_eq!(playing.lifecycle, WakeLifecycle::Playing);
    assert_eq!(playing.plans.len(), 1);
    assert_eq!(playing.plans[0].plan_id, body_plan.plan_id);
    assert_eq!(playing.plans[0].active_play_id, Some(play.active_play_id));

    let second = BodyPlayIdentity::bind(&body_plan, 2);
    assert_eq!(
        playing.body_play_started(&body_plan, &second, SignId::from("sign/parallel")),
        Err(BodyLifecycleError::InvalidTransition)
    );
}

#[test]
fn body_plan_requires_the_complete_current_workset_exactly_once() {
    let wake = two_form_wake();
    let dashboard = resident("dashboard");
    let service = resident("service");
    let dashboard_partition = BodyFormPlan {
        form: dashboard.clone(),
        plan: plan(&dashboard, "dashboard"),
    };
    assert_eq!(
        BodyPlan::seal(&wake, vec![dashboard_partition.clone()]),
        Err(BodyPlanError::MissingForm)
    );
    assert_eq!(
        BodyPlan::seal(
            &wake,
            vec![dashboard_partition.clone(), dashboard_partition]
        ),
        Err(BodyPlanError::DuplicateForm)
    );
    let unowned = resident("unowned");
    assert_eq!(
        BodyPlan::seal(
            &wake,
            vec![
                BodyFormPlan {
                    form: dashboard.clone(),
                    plan: plan(&dashboard, "dashboard"),
                },
                BodyFormPlan {
                    form: unowned.clone(),
                    plan: plan(&unowned, "unowned"),
                },
            ],
        ),
        Err(BodyPlanError::UnexpectedForm)
    );

    let only_service = Body::born(
        service.source_document_id.clone(),
        service.checked_form_id.clone(),
        9,
        SignId::from("sign/other-born"),
    )
    .unwrap()
    .wake(1, SignId::from("sign/other-woke"))
    .unwrap()
    .1;
    let stale = BodyPlan::seal(
        &only_service,
        vec![BodyFormPlan {
            form: service.clone(),
            plan: plan(&service, "service"),
        }],
    )
    .unwrap();
    assert_eq!(stale.validate_for(&wake), Err(BodyPlanError::WrongBody));
}

#[test]
fn legacy_single_plan_validation_uses_current_workset_not_seed_provenance() {
    let seed = resident("seed");
    let replacement = resident("replacement");
    let body = Body::born(
        seed.source_document_id,
        seed.checked_form_id,
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .remove_form(&resident("seed"), SignId::from("sign/remove-seed"))
    .unwrap()
    .admit_form(replacement.clone(), SignId::from("sign/add-replacement"))
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/woke")).unwrap().1;
    wake.plan_ready(
        &plan(&replacement, "replacement"),
        SignId::from("sign/planned"),
    )
    .unwrap();
}

#[test]
fn workload_change_replaces_the_plan_and_play_without_replacing_the_wake() {
    let seed = resident("dashboard");
    let service = resident("service");
    let (awake_body, wake) = Body::born(
        seed.source_document_id.clone(),
        seed.checked_form_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .admit_form(service.clone(), SignId::from("sign/admit-service"))
    .unwrap()
    .wake(1, SignId::from("sign/woke"))
    .unwrap();
    let initial_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: seed.clone(),
                plan: plan(&seed, "dashboard"),
            },
            BodyFormPlan {
                form: service.clone(),
                plan: plan(&service, "service"),
            },
        ],
    )
    .unwrap();
    let initial_play = BodyPlayIdentity::bind(&initial_plan, 1);
    let playing = wake
        .body_plan_ready(&initial_plan, SignId::from("sign/initial-plan"))
        .unwrap()
        .body_play_started(
            &initial_plan,
            &initial_play,
            SignId::from("sign/initial-play"),
        )
        .unwrap();

    let recorder = resident("recorder");
    let changed_body = awake_body
        .admit_form(recorder.clone(), SignId::from("sign/admit-recorder"))
        .unwrap();
    let changed = playing
        .workload_changed(&changed_body, SignId::from("sign/workload-changed"))
        .unwrap();
    assert_eq!(changed.wake_id, wake.wake_id);
    assert_eq!(changed.lifecycle, WakeLifecycle::Unsatisfied);
    assert_eq!(
        initial_plan.validate_for(&changed),
        Err(BodyPlanError::StaleWorkload)
    );

    let replacement = BodyPlan::seal(
        &changed,
        vec![
            BodyFormPlan {
                form: seed.clone(),
                plan: plan(&seed, "dashboard-2"),
            },
            BodyFormPlan {
                form: service.clone(),
                plan: plan(&service, "service-2"),
            },
            BodyFormPlan {
                form: recorder.clone(),
                plan: plan(&recorder, "recorder"),
            },
        ],
    )
    .unwrap();
    let replacement_play = BodyPlayIdentity::bind(&replacement, 2);
    let replaced = changed
        .body_plan_ready(&replacement, SignId::from("sign/replacement-plan"))
        .unwrap()
        .body_play_started(
            &replacement,
            &replacement_play,
            SignId::from("sign/replacement-play"),
        )
        .unwrap();
    assert_eq!(replaced.wake_id, wake.wake_id);
    assert_eq!(replaced.lifecycle, WakeLifecycle::Playing);
    assert_eq!(replaced.plans.len(), 2);
    assert_eq!(
        replaced.plans[0].state,
        conduit_body::WakePlanState::Superseded
    );
    assert_eq!(
        replaced.plans[1].state,
        conduit_body::WakePlanState::Playing
    );
}
