//! CPU lifecycle custody, not provider execution or a live remote carrier.
use super::*;
use alloc::vec;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership, BodyPlayIdentity,
    BodyPlotPlan, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::*;

fn fragment(host: &HostId, boot: &BootId) -> PlanFragment {
    PlanFragment {
        plan_id: "".into(),
        fragment_id: "".into(),
        source_document_id: "source/work".into(),
        checked_plot_id: "checked/work".into(),
        expanded_plot_id: "expanded/work".into(),
        completion_policy: PlanCompletionPolicy::Live,
        realization_backs: vec![],
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(1),
        placements: vec![],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 0,
            byte_capacity: 0,
        },
        plan_fragments: vec![],
    }
}
fn fixture() -> (BodyLifecycleSession, BodyPlotPlan, HostId, BootId) {
    let host: HostId = "host/local".into();
    let boot: BootId = "boot/local".into();
    let resident = ResidentPlot::new("source/work".into(), "checked/work".into());
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Test".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: resident.source_document_id.clone(),
            checked_plot_id: resident.checked_plot_id.clone(),
            expanded_plot_id: "expanded/work".into(),
        },
        vec![fragment(&host, &boot)],
    );
    (
        BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap(),
        BodyPlotPlan {
            plot: resident,
            plan,
        },
        host,
        boot,
    )
}

#[test]
fn exact_plot_producer_requires_current_body_play_and_host_fragment() {
    let (mut session, partition, host, boot) = fixture();
    session.propose(vec![partition], &host, &boot).unwrap();
    let current = session.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |sequence| bind_sign(&host, &boot, Some(&play.active_play_id), sequence).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    session.started(&host, &boot, play, wake).unwrap();
    let current = session.realization().unwrap();
    let plan = &current.plan.plots[0].plan;
    let basis = FaceBasis {
        body_id: Some(current.plan.body_id.clone()),
        wake_id: Some(current.wake.wake_id.clone()),
        source_document_id: Some(plan.source_document_id.clone()),
        checked_plot_id: Some(plan.checked_plot_id.clone()),
        expanded_plot_id: Some(plan.expanded_plot_id.clone()),
        plan_id: Some(plan.plan_id.clone()),
        active_play_id: Some(current.play.as_ref().unwrap().active_play_id.clone()),
        sign_ids: vec![],
    };
    let face = Face::new(
        1,
        basis,
        vec![crate::FaceSubject {
            identity: "plot/producer".into(),
            role: crate::FaceRole::Plot,
            name: "Producer".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let owner = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(1),
        profile: "profile/test".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    validation::validate_workload_basis(&session, &face, &owner).unwrap();
    let mut wrong = face.clone();
    wrong.basis.plan_id = Some(current.plan.plan_id.clone());
    assert!(validation::validate_workload_basis(&session, &wrong, &owner).is_err());
    for field in 0..6 {
        let mut wrong = face.clone();
        match field {
            0 => wrong.basis.active_play_id = None,
            1 => wrong.basis.source_document_id = Some("source/foreign".into()),
            2 => wrong.basis.checked_plot_id = None,
            3 => wrong.basis.expanded_plot_id = None,
            4 => wrong.basis.wake_id = None,
            _ => wrong.basis.active_play_id = Some("play/foreign".into()),
        }
        assert!(validation::validate_workload_basis(&session, &wrong, &owner).is_err());
    }
    let mut stale = owner.clone();
    stale.boot_id = "boot/foreign".into();
    assert!(validation::validate_workload_basis(&session, &face, &stale).is_err());
    let terminated = session.realization().unwrap().play.clone().unwrap();
    session.lull(&host, &boot, Some(&terminated)).unwrap();
    assert!(validation::validate_workload_basis(&session, &face, &owner).is_err());
}
