use super::*;
use crate::FormCandidate;
use conduit_body::{BodyWorkset, ResidentForm, WakePlanState};
use conduit_core::{BaseImplementationId, BootId, HostId};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_host::StdHost;

#[test]
fn unstarted_proposals_preserve_the_wake_without_inventing_play_events() {
    let (candidate, expanded) = form();
    let on_a = planned_form(&candidate, &expanded, "host/a", "boot/a");
    let body = Body::born_with_forms(
        BodyWorkset::one(on_a.form.clone()).unwrap(),
        1,
        "sign/proposal-born".into(),
    )
    .unwrap();
    let mut session =
        BodyPlanningSession::prepare(&body, 1, "sign/proposal-wake".into(), vec![on_a]).unwrap();
    let original_wake = session.wake().clone();
    let original_plan = session.current_plan().clone();
    assert_eq!(original_wake.lifecycle, WakeLifecycle::AwaitingPlan);
    assert!(original_wake.plans.is_empty());
    session
        .mark_current_unsatisfied("sign/proposed-host-left".into())
        .unwrap();
    assert_eq!(session.wake(), &original_wake);
    assert_eq!(session.current_plan(), &original_plan);
    assert_eq!(
        session.snapshot().unavailable_proposal_sign_id,
        Some("sign/proposed-host-left".into())
    );
    let on_b = planned_form(&candidate, &expanded, "host/b", "boot/b");
    session.replace_proposal(vec![on_b.clone()]).unwrap();
    assert!(session.snapshot().unavailable_proposal_sign_id.is_none());
    assert_eq!(session.wake(), &original_wake);
    assert_eq!(session.plan(&original_plan.plan_id), Some(&original_plan));
    let snapshot = session.snapshot();
    assert!(session.replace_proposal(vec![on_b]).is_err());
    assert_eq!(session.snapshot(), snapshot);
    assert!(session.replace_proposal(Vec::new()).is_err());
    assert_eq!(session.snapshot(), snapshot);
}

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
