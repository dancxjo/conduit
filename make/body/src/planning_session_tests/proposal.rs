use conduit_body::{
    Body, BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, BodyPlanError,
    BodyPlanningSession, BodyPlanningTransition, BodyPlotPlan, BodyWorkset, ResidentPlot,
    WakeLifecycle, WakePlanState,
};
use conduit_core::{BaseImplementationId, BootId, HostId, SignId};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_host::StdHost;

#[test]
fn unstarted_proposals_preserve_the_wake_without_inventing_play_events() {
    let expanded = super::hello_plot();
    let on_a = planned_plot(&expanded, "host/a", "boot/a");
    let body = Body::born_with_plots(
        BodyWorkset::one(on_a.plot.clone()).unwrap(),
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
    let on_b = planned_plot(&expanded, "host/b", "boot/b");
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

fn planned_plot(
    expanded: &conduit_plot::ExpandedCanonicalPlot,
    host_id: &str,
    boot_id: &str,
) -> BodyPlotPlan {
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
    BodyPlotPlan {
        plot: ResidentPlot::new(
            expanded.source_document_id.clone(),
            expanded.checked_plot_id.clone(),
        ),
        plan,
    }
}

#[test]
fn joined_host_offer_replans_one_body_without_erasing_plan_history() {
    let expanded = super::hello_plot();
    let on_a = planned_plot(&expanded, "host/a", "boot/a");
    let resident = on_a.plot.clone();
    let body = Body::born_with_plots(
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

    let on_b = planned_plot(&expanded, "host/b", "boot/b");
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
    assert!(session.current_plan().plots[0]
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
    let expanded = super::hello_plot();
    let on_a = planned_plot(&expanded, "host/a", "boot/a");
    let body = Body::born_with_plots(
        BodyWorkset::one(on_a.plot.clone()).unwrap(),
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

#[test]
fn mask_replan_changes_plan_play_without_changing_body_or_authored_plots() {
    let expanded = super::hello_plot();
    let body_plot = planned_plot(&expanded, "host/body", "boot/body");
    let resident = body_plot.plot.clone();
    let body = Body::born_with_plots(
        BodyWorkset::one(resident.clone()).unwrap(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let mask_plans = conduit_patchbay_workbench_conformance::patchbay_mask_plans().unwrap();
    let renderer = |plan: &conduit_core::Plan| {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .last()
            .unwrap()
            .placement_id
            .clone()
    };
    let selector = BodyFaceSelector {
        plot: Some(resident),
        source_placement_id: None,
    };
    let graphical = BodyMaskChainPlan {
        stage_placement_ids: vec![renderer(&mask_plans.direct)],
        plan: mask_plans.direct,
    };
    let speech = BodyMaskChainPlan {
        stage_placement_ids: vec![renderer(&mask_plans.recursive)],
        plan: mask_plans.recursive,
    };
    let initial_topology = BodyMaskTopology {
        face: selector.clone(),
        chains: vec![graphical.clone()],
    };
    let mut session = BodyPlanningSession::prepare_with_masks(
        &body,
        2,
        SignId::from("sign/woke"),
        vec![body_plot.clone()],
        vec![initial_topology],
    )
    .unwrap();
    let initial_plan = session.current_plan().clone();
    let initial_plot = initial_plan.plots[0].clone();
    session
        .replace_proposal_with_masks(
            vec![body_plot],
            vec![BodyMaskTopology {
                face: selector,
                chains: vec![graphical, speech],
            }],
        )
        .unwrap();
    assert_eq!(session.body().body_id, body.body_id);
    assert_eq!(session.current_plan().plots, vec![initial_plot]);
    assert_ne!(session.current_plan().plan_id, initial_plan.plan_id);
    assert_eq!(session.current_plan().mask_topologies[0].chains.len(), 2);
    assert_eq!(session.plan(&initial_plan.plan_id), Some(&initial_plan));

    let current = session.current_plan();
    let mut body_scoped = current.mask_topologies[0].clone();
    body_scoped.face = BodyFaceSelector {
        plot: None,
        source_placement_id: None,
    };
    let body_scoped_plan = BodyPlan::seal_with_masks(
        session.wake(),
        current.plots.clone(),
        vec![body_scoped.clone()],
    )
    .unwrap();
    assert_ne!(body_scoped_plan.plan_id, current.plan_id);
    assert!(body_scoped_plan.verify_seal().is_ok());

    let mut planned_source = current.mask_topologies[0].clone();
    planned_source.face.source_placement_id = Some(
        current.plots[0].plan.fragments[0].placements[0]
            .placement_id
            .clone(),
    );
    let planned_source_plan =
        BodyPlan::seal_with_masks(session.wake(), current.plots.clone(), vec![planned_source])
            .unwrap();
    assert_ne!(planned_source_plan.plan_id, current.plan_id);
    assert!(planned_source_plan.verify_seal().is_ok());

    let mut forged = body_scoped_plan.clone();
    forged.mask_topologies[0].face.source_placement_id = Some("invented/body-placement".into());
    assert_eq!(forged.verify_seal(), Err(BodyPlanError::InvalidMaskChain));

    body_scoped.face.source_placement_id = Some("invented/body-placement".into());
    assert_eq!(
        BodyPlan::seal_with_masks(
            session.wake(),
            current.plots.clone(),
            vec![body_scoped.clone()]
        ),
        Err(BodyPlanError::InvalidMaskChain)
    );
    body_scoped.face.plot = Some(current.plots[0].plot.clone());
    assert_eq!(
        BodyPlan::seal_with_masks(session.wake(), current.plots.clone(), vec![body_scoped]),
        Err(BodyPlanError::InvalidMaskChain)
    );
}
