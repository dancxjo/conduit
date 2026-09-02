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
