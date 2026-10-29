use conduit_body::{
    Body, BodyPlan, BodyPlanError, BodyPlanEvidence, BodyPlanSeal, BodyPlanSealRefusal as E,
    BodyPlayIdentity, BodyPlotPlan, BodyWorkset, ResidentPlot,
};
#[path = "../../core/tests/common/sealed_state.rs"]
mod fixture;

fn prepared() -> (conduit_body::Wake, BodyPlan) {
    let core = fixture::seal(fixture::fragment());
    let plot = ResidentPlot::new(
        core.source_document_id.clone(),
        core.checked_plot_id.clone(),
    );
    let body =
        Body::born_with_plots(BodyWorkset::one(plot.clone()).unwrap(), 1, "birth".into()).unwrap();
    let wake = body.wake(2, "wake".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan: core }]).unwrap();
    (wake, plan)
}
#[test]
fn immutable_seal_uses_the_same_ordinary_wake_and_play_transitions() {
    let (wake, plan) = prepared();
    let seal = BodyPlanSeal::capture(&plan).unwrap();
    let rich_play = BodyPlayIdentity::bind(&plan, 7);
    let compact_play = BodyPlayIdentity::bind(&seal, 7);
    assert_eq!(rich_play, compact_play);
    let rich_ready = wake.body_plan_ready(&plan, "ready".into()).unwrap();
    let compact_ready = wake.body_plan_ready(&seal, "ready".into()).unwrap();
    assert_eq!(rich_ready, compact_ready);
    assert_eq!(
        rich_ready
            .body_play_started(&plan, &rich_play, "started".into())
            .unwrap(),
        compact_ready
            .body_play_started(&seal, &compact_play, "started".into())
            .unwrap()
    );
    let part = &seal.plots()[0];
    let original = &plan.plots[0].plan;
    assert_eq!(part.plan_id(), &original.plan_id);
    assert_eq!(
        part.fragments()[0].fragment_id(),
        &original.fragments[0].fragment_id
    );
    assert_eq!(
        part.fragments()[0].host_id(),
        &original.fragments[0].host_id
    );
    assert_eq!(
        part.fragments()[0].boot_id(),
        &original.fragments[0].boot_id
    );
    assert_eq!(
        part.fragments()[0].offer_generation(),
        original.fragments[0].offer_generation
    );
}
#[test]
fn dropping_or_mutating_the_rich_graph_does_not_change_the_captured_evidence() {
    let (wake, mut plan) = prepared();
    let seal = BodyPlanSeal::capture(&plan).unwrap();
    let old_id = plan.plan_id.clone();
    plan.plots[0].plan.fragments[0].offer_generation.0 += 1;
    assert!(BodyPlanSeal::capture(&plan).is_err());
    drop(plan);
    assert_eq!(seal.plan_id(), &old_id);
    assert_eq!(seal.validate_for(&wake), Ok(()));
    let mut other = wake.clone();
    other.workload_revision += 1;
    assert_eq!(seal.validate_for(&other), Err(BodyPlanError::StaleWorkload));
    let other_body = Body::born_with_plots(wake.workset.clone(), 3, "other".into()).unwrap();
    let other_wake = other_body.wake(4, "another".into()).unwrap().1;
    assert_eq!(
        seal.validate_for(&other_wake),
        Err(BodyPlanError::WrongBody)
    );
    let mut changed_wake = wake.clone();
    changed_wake.wake_id = other_wake.wake_id;
    assert_eq!(
        seal.validate_for(&changed_wake),
        Err(BodyPlanError::WrongWake)
    );
}
#[test]
fn invalid_identity_and_excess_fragment_capacity_cannot_be_sealed() {
    let (_, mut plan) = prepared();
    plan.plan_id = "forged".into();
    assert_eq!(
        BodyPlanSeal::capture(&plan),
        Err(E::Invalid(BodyPlanError::InvalidIdentity))
    );
    let fragments = (0..=conduit_body::MAX_BODY_SEAL_FRAGMENTS)
        .map(|i| {
            let mut f = fixture::fragment();
            f.host_id = format!("host/{i}").into();
            f.placements.clear();
            f.states.clear();
            f.startup_order.clear();
            f
        })
        .collect();
    let core = conduit_core::seal_plan(
        conduit_core::PlotIdentity {
            source_document_id: "source".into(),
            checked_plot_id: "checked".into(),
            expanded_plot_id: "expanded".into(),
        },
        fragments,
    );
    let (wake, _) = prepared();
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: wake.workset.plots()[0].clone(),
            plan: core,
        }],
    )
    .unwrap();
    assert_eq!(BodyPlanSeal::capture(&plan), Err(E::Capacity));
}
#[test]
fn retained_mask_geometry_is_read_only_and_preserves_its_core_scope() {
    let (wake, plan) = prepared();
    let mask = conduit_body::BodyMaskTopology {
        face: conduit_body::BodyFaceSelector {
            plot: None,
            source_placement_id: None,
        },
        chains: vec![conduit_body::BodyMaskChainPlan {
            plan: fixture::seal(fixture::fragment()),
            stage_placement_ids: vec!["placement".into()],
        }],
    };
    let plan = BodyPlan::seal_with_masks(&wake, plan.plots, vec![mask.clone()]).unwrap();
    let seal = BodyPlanSeal::capture(&plan).unwrap();
    assert_eq!(seal.masks()[0].face(), &mask.face);
    assert_eq!(
        seal.masks()[0].chains()[0].stages(),
        &mask.chains[0].stage_placement_ids
    );
    assert_eq!(
        seal.masks()[0].chains()[0].plan().plan_id(),
        &mask.chains[0].plan.plan_id
    );
    assert_eq!(seal.validate_for(&wake), Ok(()));
}

#[test]
fn recursive_execution_is_refused_instead_of_discarding_child_plan_material() {
    use conduit_core::*;
    let mut child = fixture::fragment();
    child.fore_ports = [
        ("in", "next", PortDirection::Input),
        ("out", "current", PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| PlannedForePort {
        front_port_id: port_id(front),
        direction,
        placement_id: "placement".into(),
        gear_port_id: port_id(gear),
        value_kind: kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    })
    .collect();
    let child = fixture::seal(child);
    let activation = PlannedActivation {
        activation_id: "child".into(),
        owner_placement_id: "placement".into(),
        selected_plan_id: child.plan_id.clone(),
        selected_plan: Box::new(child),
        input: PlannedActivationFront {
            front_port_id: port_id("in"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        output: PlannedActivationFront {
            front_port_id: port_id("out"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        limits: PlannedActivationLimits {
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 1,
            maximum_items: 1,
        },
        terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
    };
    let core = seal_plan_with_activations(
        PlotIdentity {
            source_document_id: "source".into(),
            checked_plot_id: "checked".into(),
            expanded_plot_id: "expanded".into(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![activation],
        vec![fixture::fragment()],
    );
    assert!(verify_plan(&core));
    let (wake, _) = prepared();
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: wake.workset.plots()[0].clone(),
            plan: core,
        }],
    )
    .unwrap();
    assert_eq!(BodyPlanSeal::capture(&plan), Err(E::NestedExecution));
}
