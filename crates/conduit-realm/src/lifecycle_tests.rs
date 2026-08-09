use alloc::{format, vec};

use conduit_core::{bind_active_play, CheckedFormId, EvidenceId, Plan, PlanId, SourceDocumentId};

use super::{
    ActivationLifecycle, ActivationLifecycleEvent, ActivationPlanState, DeploymentLifecycleEvent,
    DeploymentState, RealmDeployment, RealmId, RealmLifecycleError, MAX_ACTIVATION_EVIDENCE,
    MAX_ACTIVATION_PLANS,
};

fn plan(source: &str, checked: &str, identity: &str) -> Plan {
    Plan {
        source_document_id: SourceDocumentId::from(source),
        checked_form_id: CheckedFormId::from(checked),
        expanded_form_id: "expanded".into(),
        plan_id: PlanId::from(identity),
        fragments: vec![],
    }
}

fn installed() -> RealmDeployment {
    RealmDeployment::install(
        RealmId::from("realm-a"),
        SourceDocumentId::from("source-a"),
        CheckedFormId::from("checked-a"),
        4,
        EvidenceId::from("deployed"),
    )
    .unwrap()
}

#[test]
fn deployment_precedes_activation_and_one_activation_survives_replan() {
    let deployment = installed();
    assert_eq!(deployment.state, DeploymentState::Inactive);
    let (active_deployment, activation) = deployment
        .activate(9, EvidenceId::from("activated"))
        .unwrap();
    assert_eq!(activation.lifecycle, ActivationLifecycle::AwaitingPlan);

    let plan_a = plan("source-a", "checked-a", "ignored-a");
    let awaiting_a = activation
        .plan_ready(&plan_a, EvidenceId::from("plan-a-ready"))
        .unwrap();
    let play_a = bind_active_play(&plan_a.plan_id, &"host-a".into(), &"boot-a".into(), 1);
    let playing_a = awaiting_a
        .play_started(&play_a, EvidenceId::from("play-a-started"))
        .unwrap();
    let unsatisfied = playing_a
        .became_unsatisfied(&plan_a.plan_id, EvidenceId::from("plan-a-unsatisfied"))
        .unwrap();

    let plan_b = plan("source-a", "checked-a", "ignored-b");
    let awaiting_b = unsatisfied
        .plan_ready(&plan_b, EvidenceId::from("plan-b-ready"))
        .unwrap();
    let play_b = bind_active_play(&plan_b.plan_id, &"host-b".into(), &"boot-b".into(), 2);
    let playing_b = awaiting_b
        .play_started(&play_b, EvidenceId::from("play-b-started"))
        .unwrap();

    assert_eq!(playing_b.activation_id, activation.activation_id);
    assert_eq!(playing_b.deployment_id, deployment.deployment_id);
    assert_ne!(plan_a.plan_id, plan_b.plan_id);
    assert_ne!(play_a.active_play_id, play_b.active_play_id);
    assert_eq!(playing_b.plans.len(), 2);
    assert_eq!(playing_b.plans[0].state, ActivationPlanState::Superseded);
    assert_eq!(
        playing_b.plans[0].active_play_id,
        Some(play_a.active_play_id)
    );
    assert_eq!(
        playing_b.plans[1].active_play_id,
        Some(play_b.active_play_id)
    );
    assert!(matches!(
        &playing_b.events[4],
        ActivationLifecycleEvent::Replanned {
            prior_plan_id,
            replacement_plan_id,
            evidence_id,
        } if prior_plan_id == &plan_a.plan_id
            && replacement_plan_id == &plan_b.plan_id
            && evidence_id.as_str() == "plan-b-ready"
    ));

    let deactivated = playing_b
        .deactivate(EvidenceId::from("deactivated"))
        .unwrap();
    let retained = active_deployment
        .retain_after_activation(&deactivated, EvidenceId::from("retained"))
        .unwrap();
    assert_eq!(retained.state, DeploymentState::Inactive);
    let removed = retained.undeploy(EvidenceId::from("undeployed")).unwrap();
    assert_eq!(removed.state, DeploymentState::Undeployed);
    assert!(matches!(
        removed.events.last(),
        Some(DeploymentLifecycleEvent::Undeployed { evidence_id })
            if evidence_id.as_str() == "undeployed"
    ));
}

#[test]
fn same_plan_observation_preserves_plan_play_and_activation() {
    let (deployment, activation) = installed()
        .activate(1, EvidenceId::from("activated"))
        .unwrap();
    let exact_plan = plan("source-a", "checked-a", "plan");
    let awaiting = activation
        .plan_ready(&exact_plan, EvidenceId::from("planned"))
        .unwrap();
    let play = bind_active_play(&exact_plan.plan_id, &"host".into(), &"boot".into(), 8);
    let active = awaiting
        .play_started(&play, EvidenceId::from("playing"))
        .unwrap();
    let observed = active
        .same_plan_observed(&exact_plan.plan_id, EvidenceId::from("route-selected"))
        .unwrap();

    assert_eq!(observed.activation_id, active.activation_id);
    assert_eq!(observed.plans, active.plans);
    assert_eq!(observed.lifecycle, ActivationLifecycle::Active);
    assert!(matches!(
        observed.events.last(),
        Some(ActivationLifecycleEvent::SamePlanObserved { plan_id, evidence_id })
            if plan_id == &exact_plan.plan_id && evidence_id.as_str() == "route-selected"
    ));
    assert_eq!(
        deployment.state,
        DeploymentState::Active {
            activation_id: active.activation_id
        }
    );
}

#[test]
fn stale_plan_play_activation_and_active_undeploy_fail_closed() {
    let installed = installed();
    let (deployment, activation) = installed
        .activate(1, EvidenceId::from("activated"))
        .unwrap();
    assert_eq!(
        deployment.undeploy(EvidenceId::from("too-soon")),
        Err(RealmLifecycleError::InvalidTransition)
    );

    let stale = plan("other-source", "checked-a", "stale");
    assert_eq!(
        activation.plan_ready(&stale, EvidenceId::from("stale-plan")),
        Err(RealmLifecycleError::StalePlan)
    );

    let exact = plan("source-a", "checked-a", "exact");
    let awaiting = activation
        .plan_ready(&exact, EvidenceId::from("planned"))
        .unwrap();
    let mut drifted_event = awaiting.clone();
    if let ActivationLifecycleEvent::PlanReady { plan_id, .. } = &mut drifted_event.events[1] {
        *plan_id = PlanId::from("other-plan");
    }
    assert_eq!(
        drifted_event.validate(),
        Err(RealmLifecycleError::InvalidTransition)
    );
    let wrong_play = bind_active_play(&"wrong-plan".into(), &"host".into(), &"boot".into(), 1);
    assert_eq!(
        awaiting.play_started(&wrong_play, EvidenceId::from("wrong-play")),
        Err(RealmLifecycleError::StalePlay)
    );

    let other_activation = installed
        .activate(2, EvidenceId::from("other-activation"))
        .unwrap()
        .1
        .deactivate(EvidenceId::from("other-deactivated"))
        .unwrap();
    assert_eq!(
        deployment.retain_after_activation(&other_activation, EvidenceId::from("mismatch")),
        Err(RealmLifecycleError::MismatchedActivation)
    );

    let mut wrong_realm = activation;
    wrong_realm.realm_id = RealmId::from("other-realm");
    assert_eq!(
        wrong_realm.validate(),
        Err(RealmLifecycleError::InvalidIdentity)
    );
}

#[test]
fn evidence_and_plan_histories_are_finite_and_duplicate_evidence_is_rejected() {
    let (_, activation) = installed()
        .activate(1, EvidenceId::from("activated"))
        .unwrap();
    assert_eq!(
        activation.deactivate(EvidenceId::from("activated")),
        Err(RealmLifecycleError::DuplicateEvidence)
    );

    let exact = plan("source-a", "checked-a", "bounded-plan");
    let awaiting = activation
        .plan_ready(&exact, EvidenceId::from("planned"))
        .unwrap();
    let play = bind_active_play(&exact.plan_id, &"host".into(), &"boot".into(), 1);
    let mut current = awaiting
        .play_started(&play, EvidenceId::from("playing"))
        .unwrap();
    for index in 3..MAX_ACTIVATION_EVIDENCE {
        current = current
            .same_plan_observed(
                &exact.plan_id,
                EvidenceId::from(format!("evidence-{index}")),
            )
            .unwrap();
    }
    assert_eq!(
        current.same_plan_observed(&exact.plan_id, EvidenceId::from("overflow")),
        Err(RealmLifecycleError::EvidenceCapacityExhausted)
    );
}

#[test]
fn activation_rejects_more_than_its_admitted_plan_history() {
    let (_, mut activation) = installed()
        .activate(1, EvidenceId::from("activated"))
        .unwrap();
    for index in 0..MAX_ACTIVATION_PLANS {
        let exact = plan("source-a", "checked-a", &format!("plan-{index}"));
        activation = activation
            .plan_ready(&exact, EvidenceId::from(format!("plan-{index}-ready")))
            .unwrap();
        let play = bind_active_play(&exact.plan_id, &"host".into(), &"boot".into(), index as u64);
        activation = activation
            .play_started(&play, EvidenceId::from(format!("plan-{index}-playing")))
            .unwrap();
        activation = activation
            .became_unsatisfied(
                &exact.plan_id,
                EvidenceId::from(format!("plan-{index}-unsatisfied")),
            )
            .unwrap();
    }
    let overflow = plan("source-a", "checked-a", "overflow");
    assert_eq!(
        activation.plan_ready(&overflow, EvidenceId::from("overflow-ready")),
        Err(RealmLifecycleError::PlanCapacityExhausted)
    );
}
