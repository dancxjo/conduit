use super::*;
use crate::FormCandidate;
use conduit_body::{BodyWorkset, ResidentForm, WakePlanState};
use conduit_core::{BaseImplementationId, BootId, HostId};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_host::StdHost;

fn form() -> (FormCandidate, conduit_form::ExpandedCanonicalForm) {
    let candidate = FormCandidate::from_source(
        "Hello",
        "forms/hello/main.conduit",
        include_str!("../../../../../forms/hello/main.conduit"),
        "canonical test Form",
        SignId::from("sign/form-reviewed"),
        1,
    )
    .unwrap();
    let expanded = candidate.editor().unwrap().expand_form("hello").unwrap();
    (candidate, expanded)
}

fn planned_form(
    candidate: &FormCandidate,
    expanded: &conduit_form::ExpandedCanonicalForm,
    host_id: &str,
    boot_id: &str,
) -> BodyFormPlan {
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.host_id = HostId::from(host_id);
    advertisement.boot_id = BootId::from(boot_id);
    let placements = default_expanded_placements(expanded, &[advertisement.clone()]).unwrap();
    let plan = plan_expanded_canonical(
        expanded,
        &[advertisement],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    BodyFormPlan {
        form: ResidentForm::new(
            candidate.source_document_id.clone(),
            candidate.checked_form_id.clone(),
        ),
        plan,
    }
}

#[test]
fn joined_host_offer_replans_one_body_without_erasing_plan_history() {
    let (candidate, expanded) = form();
    let on_a = planned_form(&candidate, &expanded, "host/a", "boot/a");
    let resident = on_a.form.clone();
    let body = Body::born_with_forms(
        BodyWorkset::one(resident).unwrap(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let original_body_id = body.body_id.clone();
    let mut session = BodyPlanningSession::start(
        &body,
        2,
        SignId::from("sign/woke"),
        vec![on_a],
        SignId::from("sign/plan-a-ready"),
        1,
        SignId::from("sign/play-a-started"),
    )
    .unwrap();
    let prior_plan_id = session.current_plan().plan_id.clone();

    let on_b = planned_form(&candidate, &expanded, "host/b", "boot/b");
    session
        .replan(
            vec![on_b],
            BodyPlanningTransition {
                unsatisfied_sign_id: Some(SignId::from("sign/host-b-selected")),
                plan_ready_sign_id: SignId::from("sign/plan-b-ready"),
                play_sequence: 2,
                play_started_sign_id: SignId::from("sign/play-b-started"),
            },
        )
        .unwrap();

    assert_eq!(session.body().body_id, original_body_id);
    assert_ne!(session.current_plan().plan_id, prior_plan_id);
    assert!(session.current_plan().forms[0]
        .plan
        .fragments
        .iter()
        .all(|fragment| fragment.host_id.as_str() == "host/b"));
    assert_eq!(session.wake().plans[0].state, WakePlanState::Superseded);
    assert_eq!(session.wake().plans[1].state, WakePlanState::Playing);
    assert_eq!(session.plan(&prior_plan_id).unwrap().plan_id, prior_plan_id);
    assert_eq!(session.snapshot().historical_plan_ids.len(), 2);
}

#[test]
fn selected_host_loss_is_machine_readable_and_keeps_the_plan() {
    let (candidate, expanded) = form();
    let on_a = planned_form(&candidate, &expanded, "host/a", "boot/a");
    let body = Body::born_with_forms(
        BodyWorkset::one(on_a.form.clone()).unwrap(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let mut session = BodyPlanningSession::start(
        &body,
        2,
        SignId::from("sign/woke"),
        vec![on_a],
        SignId::from("sign/plan-ready"),
        1,
        SignId::from("sign/play-started"),
    )
    .unwrap();
    let plan_id = session.current_plan().plan_id.clone();

    session
        .mark_current_unsatisfied(SignId::from("sign/selected-host-left"))
        .unwrap();

    assert_eq!(session.wake().lifecycle, WakeLifecycle::Unsatisfied);
    assert_eq!(session.wake().plans[0].state, WakePlanState::Unsatisfied);
    assert_eq!(session.current_plan().plan_id, plan_id);
}
