use super::*;
use conduit_body::BodyId;
use conduit_core::{
    bind_active_play, ActivePlayId, BootId, CapabilityId, CheckedFormId, ExpandedFormId,
    ImplementationId, PlacementId, PlanId, SignId, SourceDocumentId,
};
use conduit_presentation::{PresentationBasis, PresentationRole, PresentationSubject};

fn stage(id: &str, implementation: &str, input: &str, output: Option<&str>) -> PresenterStage {
    PresenterStage {
        stage_id: id.into(),
        placement_id: PlacementId::from(format!("placement/{id}")),
        capability_id: CapabilityId::from(format!("capability/{id}")),
        implementation_id: ImplementationId::from(implementation),
        host_id: conduit_core::HostId::from(format!("host/{id}")),
        boot_id: BootId::from(format!("boot/{id}")),
        input_kind: input.into(),
        output_kind: output.map(Into::into),
        capacity_cost: 4,
        available: true,
        authorized: true,
    }
}
fn chain(id: &str, implementation: &str) -> PresenterChain {
    PresenterChain {
        chain_id: id.into(),
        stages: vec![stage(id, implementation, "presentation/structured@1", None)],
        manifestation_id: format!("manifestation/{id}"),
    }
}
fn topology(chains: Vec<PresenterChain>) -> PresenterTopology {
    PresenterTopology::new(
        body_id("self", 0),
        SourceDocumentId::from("source/self"),
        "presentation/patchbay".into(),
        PlanId::from("plan/a"),
        ActivePlayId::from("play/a"),
        chains,
    )
    .unwrap()
}
fn body_id(label: &str, sequence: u64) -> BodyId {
    conduit_body::Body::born(
        SourceDocumentId::from(format!("source/{label}")),
        CheckedFormId::from(format!("checked/{label}")),
        sequence,
        SignId::from(format!("sign/{label}")),
    )
    .unwrap()
    .body_id
}
fn request(
    current: &PresenterTopology,
    change: PresenterTopologyChange,
) -> PresenterTopologyRequest {
    PresenterTopologyRequest {
        body_id: current.body_id.clone(),
        source_document_id: current.source_document_id.clone(),
        presentation_id: current.presentation_id.clone(),
        basis_plan_id: current.plan_id.clone(),
        change,
    }
}
fn renderer(plan: &conduit_core::Plan) -> &conduit_core::PlannedGear {
    plan.fragments
        .iter()
        .flat_map(|fragment| fragment.placements.iter())
        .last()
        .unwrap()
}

#[test]
fn patchbay_adds_and_removes_its_own_presenters_through_fresh_plan_and_play() {
    let graphical = chain("graphical", "conduit.presenter/native-graphical@1");
    let speech = chain("speech", "conduit.presenter/test-speech@1");
    let original = topology(vec![graphical.clone()]);
    let added = original
        .request_replacement(
            request(&original, PresenterTopologyChange::Add(speech.clone())),
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b"),
        )
        .unwrap();
    assert_eq!(added.current.body_id, original.body_id);
    assert_eq!(
        added.current.source_document_id,
        original.source_document_id
    );
    assert_eq!(added.current.presentation_id, original.presentation_id);
    assert_ne!(added.current.plan_id, original.plan_id);
    assert_ne!(added.current.active_play_id, original.active_play_id);
    assert_eq!(added.current.chains.len(), 2);
    assert!(added.replaced_manifestations.is_empty());
    let speech_only = added
        .current
        .request_replacement(
            request(
                &added.current,
                PresenterTopologyChange::Remove {
                    chain_id: graphical.chain_id.clone(),
                },
            ),
            PlanId::from("plan/c"),
            ActivePlayId::from("play/c"),
        )
        .unwrap();
    assert_eq!(speech_only.current.chains, vec![speech]);
    assert_eq!(
        speech_only.replaced_manifestations,
        vec!["manifestation/graphical"]
    );
    let restored = speech_only
        .current
        .request_replacement(
            request(
                &speech_only.current,
                PresenterTopologyChange::Add(PresenterChain {
                    manifestation_id: "manifestation/graphical-restored".into(),
                    ..graphical
                }),
            ),
            PlanId::from("plan/d"),
            ActivePlayId::from("play/d"),
        )
        .unwrap();
    assert_eq!(restored.current.chains.len(), 2);
}

#[test]
fn two_stage_handoff_and_reorder_are_typed_and_finite() {
    let chain = PresenterChain {
        chain_id: "spoken".into(),
        stages: vec![
            stage(
                "normalize",
                "text-normalizer",
                "presentation/structured@1",
                Some("presentation/text@1"),
            ),
            stage("speech", "test-speech", "presentation/text@1", None),
        ],
        manifestation_id: "manifestation/spoken".into(),
    };
    let current = topology(vec![chain]);
    assert_eq!(
        current.request_replacement(
            request(
                &current,
                PresenterTopologyChange::Reorder {
                    chain_id: "spoken".into(),
                    stage_ids: vec!["speech".into(), "normalize".into()]
                }
            ),
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b")
        ),
        Err(PresenterTopologyRefusal::IncompatibleType)
    );
}

#[test]
fn refusal_causes_are_distinct_and_leave_current_realization_unchanged() {
    let current = topology(vec![chain("graphical", "native")]);
    let mut stale = request(
        &current,
        PresenterTopologyChange::Add(chain("speech", "speech")),
    );
    stale.basis_plan_id = PlanId::from("plan/stale");
    assert_eq!(
        current.request_replacement(stale, PlanId::from("plan/b"), ActivePlayId::from("play/b")),
        Err(PresenterTopologyRefusal::StaleRequest)
    );
    let mut unavailable = chain("speech", "speech");
    unavailable.stages[0].available = false;
    assert_eq!(
        current.request_replacement(
            request(&current, PresenterTopologyChange::Add(unavailable)),
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b")
        ),
        Err(PresenterTopologyRefusal::UnavailablePresenter)
    );
    let mut denied = chain("speech", "speech");
    denied.stages[0].authorized = false;
    assert_eq!(
        current.request_replacement(
            request(&current, PresenterTopologyChange::Add(denied)),
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b")
        ),
        Err(PresenterTopologyRefusal::AuthorityPolicy)
    );
    let mut pressure = chain("speech", "speech");
    pressure.stages[0].capacity_cost = MAX_PRESENTER_CAPACITY;
    assert_eq!(
        current.request_replacement(
            request(&current, PresenterTopologyChange::Add(pressure)),
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b")
        ),
        Err(PresenterTopologyRefusal::ResourcePressure)
    );
    assert_eq!(current.plan_id.as_str(), "plan/a");
    assert_eq!(current.chains.len(), 1);
}

#[test]
fn cycle_overlength_lost_host_and_superseded_body_refuse_distinctly() {
    let current = topology(vec![chain("graphical", "native")]);
    let mut cycle = chain("cycle", "one");
    cycle.stages.push(cycle.stages[0].clone());
    assert_eq!(
        validate_chains(&[cycle]),
        Err(PresenterTopologyRefusal::Cycle)
    );
    let mut long = chain("long", "one");
    for index in 1..=MAX_PRESENTER_STAGES_PER_CHAIN {
        long.stages.last_mut().unwrap().output_kind = Some(format!("presentation/stage-{index}"));
        long.stages.push(stage(
            &format!("stage-{index}"),
            "filter",
            &format!("presentation/stage-{index}"),
            None,
        ));
    }
    assert_eq!(
        validate_chains(&[long]),
        Err(PresenterTopologyRefusal::ChainLengthBound)
    );
    let mut lost = chain("lost", "speech");
    lost.stages.insert(
        0,
        stage(
            "filter",
            "filter",
            "presentation/structured@1",
            Some("presentation/structured@1"),
        ),
    );
    lost.stages[1].available = false;
    assert_eq!(
        validate_chains(&[lost]),
        Err(PresenterTopologyRefusal::LostHost)
    );
    let mut wrong_body = request(
        &current,
        PresenterTopologyChange::Add(chain("speech", "speech")),
    );
    wrong_body.body_id = body_id("replaced", 1);
    assert_eq!(
        current.request_replacement(
            wrong_body,
            PlanId::from("plan/b"),
            ActivePlayId::from("play/b")
        ),
        Err(PresenterTopologyRefusal::SupersededBodyTruth)
    );
}

#[test]
fn browser_and_native_receive_one_portable_exact_topology_and_visible_controls() {
    let current = topology(vec![
        chain("graphical", "conduit.presenter/native-graphical@1"),
        chain("speech", "conduit.presenter/test-speech@1"),
    ]);
    let base = conduit_presentation::Presentation::new_with_semantics(
        7,
        PresentationBasis {
            body_id: Some(current.body_id.clone()),
            wake_id: None,
            source_document_id: Some(current.source_document_id.clone()),
            checked_form_id: Some(CheckedFormId::from("checked/self")),
            expanded_form_id: Some(ExpandedFormId::from("expanded/self")),
            plan_id: Some(current.plan_id.clone()),
            active_play_id: Some(current.active_play_id.clone()),
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "patchbay".into(),
            role: PresentationRole::Document,
            label: "Patchbay".into(),
            accessibility_name: "Patchbay".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let portable = project_presenter_topology(&base, &current).unwrap();
    portable.validate().unwrap();
    assert_eq!(portable.basis, base.basis);
    assert_eq!(
        portable.identity,
        project_presenter_topology(&base, &current)
            .unwrap()
            .identity
    );
    assert!(portable.subjects.iter().any(|subject| subject
        .accessibility_name
        .contains("Host host/graphical Boot boot/graphical")));
    for operation in ["add", "remove", "replace", "reorder", "toggle-parallel"] {
        assert!(portable
            .actions
            .iter()
            .any(|action| action.intent.contains(operation)));
    }
}

#[test]
fn public_replanning_seam_requires_exact_verified_planner_output() {
    let plans = patchbay_presenter_plans().unwrap();
    let direct = renderer(&plans.direct);
    let recursive = renderer(&plans.recursive);
    let from_placement = |id: &str, placement: &conduit_core::PlannedGear| PresenterChain {
        chain_id: "graphical".into(),
        stages: vec![PresenterStage {
            stage_id: id.into(),
            placement_id: placement.placement_id.clone(),
            capability_id: placement.capability_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            input_kind: "presentation/structured@1".into(),
            output_kind: None,
            capacity_cost: 1,
            available: true,
            authorized: true,
        }],
        manifestation_id: format!("manifestation/{id}"),
    };
    let current_play = bind_active_play(&plans.direct.plan_id, &direct.host_id, &direct.boot_id, 1);
    let current = PresenterTopology::new(
        body_id("planned", 2),
        plans.direct.source_document_id.clone(),
        "presentation/patchbay".into(),
        plans.direct.plan_id.clone(),
        current_play.active_play_id,
        vec![from_placement("direct", direct)],
    )
    .unwrap();
    let replacement_play = bind_active_play(
        &plans.recursive.plan_id,
        &recursive.host_id,
        &recursive.boot_id,
        2,
    );
    let replacement = current
        .request_replacement_from_plan(
            request(
                &current,
                PresenterTopologyChange::Replace {
                    chain_id: "graphical".into(),
                    replacement: from_placement("recursive", recursive),
                },
            ),
            &plans.recursive,
            replacement_play,
        )
        .unwrap();
    assert_eq!(replacement.current.plan_id, plans.recursive.plan_id);
    assert_eq!(
        replacement.current.chains[0].stages[0].placement_id,
        recursive.placement_id
    );
}
