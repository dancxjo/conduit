#[allow(dead_code)]
mod model_effect_common;

use conduit_ai::{
    EmbodiedModelReceipt, EmbodiedModelView, EmbodimentStage, ProposalDecisionOutcome,
    ProposalRefusal, LLM_PROPOSE_KIND,
};
use conduit_core::*;
use model_effect_common::{wired_plan, EFFECT_KIND};

#[test]
fn embodiment_observation_is_derived_from_the_sealed_plan() {
    let plan = wired_plan();
    let view = EmbodiedModelView::from_plan(
        EmbodimentStage::AuthorizedEffect,
        &plan,
        ActivePlayId::from("play/model-effect"),
        &PlacementId::from("placement/proposer"),
        &kind_id("value/text@1"),
        &kind_id(EFFECT_KIND),
        "proposal/model/plan-derived".into(),
        ProposalDecisionOutcome::Authorized {
            request_id: "request/model/effect".into(),
        },
        vec![SignId::from("sign/model/effect")],
    )
    .unwrap();

    assert_eq!(view.plan_id, plan.plan_id);
    assert_eq!(view.model_gear_identity, "placement/proposer");
    assert_eq!(
        view.model_implementation_identity,
        "implementation/placement/proposer"
    );
    assert_eq!(view.wired_outputs, vec![kind_id("llm/proposal-result@1")]);
    assert!(view.protected_effect_wired);
    assert_eq!(view.authority_id.as_deref(), Some("grant/send-message"));
    assert_eq!(
        plan.fragments[0].placements[0].kind_id.as_str(),
        LLM_PROPOSE_KIND
    );
}

#[test]
fn mutation_after_sealing_cannot_change_the_observed_situation() {
    let mut plan = wired_plan();
    plan.fragments[0].connections.clear();

    assert!(matches!(
        EmbodiedModelView::from_plan(
            EmbodimentStage::PerceptionOnly,
            &plan,
            ActivePlayId::from("play/mutated"),
            &PlacementId::from("placement/proposer"),
            &kind_id("value/text@1"),
            &kind_id(EFFECT_KIND),
            "proposal/mutated".into(),
            ProposalDecisionOutcome::Refused(conduit_ai::ProposalRefusal::UnwiredOperation),
            vec![],
        ),
        Err(conduit_ai::EmbodimentReceiptError::InvalidPlan)
    ));
}

#[test]
fn three_sealed_forms_mechanically_add_expression_then_narrow_effect_power() {
    let plans = [graph_plan(0), graph_plan(1), graph_plan(2)];
    let views = plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            let authorized = index == 2;
            EmbodiedModelView::from_plan(
                [
                    EmbodimentStage::PerceptionOnly,
                    EmbodimentStage::Expressive,
                    EmbodimentStage::AuthorizedEffect,
                ][index],
                plan,
                ActivePlayId::from(format!("play/embodied/{index}")),
                &PlacementId::from("placement/model"),
                &kind_id("value/text@1"),
                &kind_id("effect/indicator-set@1"),
                format!("proposal/embodied/{index}"),
                if authorized {
                    ProposalDecisionOutcome::Authorized {
                        request_id: "request/indicator".into(),
                    }
                } else {
                    ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
                },
                authorized
                    .then(|| SignId::from("sign/indicator"))
                    .into_iter()
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let receipt = EmbodiedModelReceipt {
        schema: "conduit.llm/embodied-body-receipt@1",
        proof_class: "hosted-integration",
        body_id: "body/embodied".into(),
        perception_value_kind: kind_id("perception/scene-summary@1"),
        state_value_kind: kind_id("robotics/battery-state@1"),
        expressive_value_kind: kind_id("value/text@1"),
        protected_effect_kind: kind_id("effect/indicator-set@1"),
        views,
        ambient_host_access: false,
    };

    receipt.validate().unwrap();
    assert_eq!(receipt.views[0].wired_outputs, Vec::<KindId>::new());
    assert_eq!(
        receipt.views[1].wired_outputs,
        vec![kind_id("value/text@1")]
    );
    assert_eq!(
        receipt.views[2].wired_outputs,
        vec![kind_id("llm/proposal-result@1"), kind_id("value/text@1")]
    );
}

fn graph_plan(stage: usize) -> Plan {
    let mut placements = vec![
        gear("placement/scene", "perception/source@1"),
        gear("placement/state", "state/source@1"),
        gear("placement/model", LLM_PROPOSE_KIND),
    ];
    placements[0].outputs = vec![port(
        "scene",
        "perception/scene-summary@1",
        PortDirection::Output,
    )];
    placements[1].outputs = vec![port(
        "state",
        "robotics/battery-state@1",
        PortDirection::Output,
    )];
    placements[2].inputs = vec![
        port("scene", "perception/scene-summary@1", PortDirection::Input),
        port("state", "robotics/battery-state@1", PortDirection::Input),
    ];
    placements[2].outputs = vec![
        port("expression", "value/text@1", PortDirection::Output),
        port("proposal", "llm/proposal-result@1", PortDirection::Output),
    ];
    let mut connections = vec![
        connection(
            "scene-model",
            "placement/scene",
            "scene",
            "placement/model",
            "scene",
            "perception/scene-summary@1",
        ),
        connection(
            "state-model",
            "placement/state",
            "state",
            "placement/model",
            "state",
            "robotics/battery-state@1",
        ),
    ];
    if stage > 0 {
        let mut presenter = gear("placement/presenter", "presentation/text@1");
        presenter.inputs = vec![port("text", "value/text@1", PortDirection::Input)];
        placements.push(presenter);
        connections.push(connection(
            "model-presenter",
            "placement/model",
            "expression",
            "placement/presenter",
            "text",
            "value/text@1",
        ));
    }
    if stage > 1 {
        let mut effect = gear("placement/indicator", "effect/indicator-set@1");
        effect.inputs = vec![port(
            "request",
            "llm/proposal-result@1",
            PortDirection::Input,
        )];
        effect.host_operations = vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from("conduit.host/present-value@1"),
            target_kind: Some(kind_id("effect/indicator-set@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: 1_024,
            maximum_output_bytes: 1_024,
        }];
        effect.authority = vec![AuthorityBinding {
            grant_id: AuthorityGrantId::from("grant/indicator-only"),
            contract_id: AuthorityContractId::from("authority/indicator-set@1"),
            host_operation_contract_id: HostOperationContractId::from(
                "conduit.host/present-value@1",
            ),
            subject_kind: kind_id("effect/indicator-set@1"),
            host_id: HostId::from("host/a"),
            boot_id: BootId::from("boot/a"),
            capability_id: CapabilityId::from("capability/placement/indicator"),
        }];
        placements.push(effect);
        connections.push(connection(
            "model-indicator",
            "placement/model",
            "proposal",
            "placement/indicator",
            "request",
            "llm/proposal-result@1",
        ));
    }
    let identity = FormIdentity {
        source_document_id: SourceDocumentId::from(format!("source/embodied/{stage}")),
        checked_form_id: CheckedFormId::from(format!("checked/embodied/{stage}")),
        expanded_form_id: ExpandedFormId::from(format!("expanded/embodied/{stage}")),
    };
    seal_plan(
        identity,
        vec![PlanFragment {
            plan_id: PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from(""),
            checked_form_id: CheckedFormId::from(""),
            expanded_form_id: ExpandedFormId::from(""),
            realization_backs: vec![],
            host_id: HostId::from("host/a"),
            boot_id: BootId::from("boot/a"),
            offer_generation: OfferGeneration(1),
            placements,
            execution_regions: vec![],
            execution_fusions: vec![],
            states: Vec::new(),
            connections,
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
        }],
    )
}

fn gear(id: &str, kind: &str) -> PlannedGear {
    PlannedGear {
        placement_id: PlacementId::from(id),
        gear_id: GearId::from("gear/model"),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("conduit.{kind}@1")),
        execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
        configuration: vec![],
        host_id: HostId::from("host/a"),
        boot_id: BootId::from("boot/a"),
        offer_generation: OfferGeneration(1),
        capability_id: CapabilityId::from(format!("capability/{id}")),
        implementation_id: ImplementationId::from("ollama/gpt-oss:20b/exact-digest"),
        artifact_id: ArtifactId::from("artifact/gpt-oss:20b"),
        realization_characteristics: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: 4096,
        },
        inputs: vec![],
        outputs: vec![],
        host_operations: vec![],
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}

fn port(id: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(id),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn connection(
    id: &str,
    source: &str,
    source_port: &str,
    sink: &str,
    sink_port: &str,
    kind: &str,
) -> PlannedConnection {
    PlannedConnection {
        connection_id: ConnectionId::from(id),
        source_placement_id: PlacementId::from(source),
        source_port_id: port_id(source_port),
        sink_placement_id: PlacementId::from(sink),
        sink_port_id: port_id(sink_port),
        value_kind: kind_id(kind),
        temporal: PortTemporal::Value,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 1,
        byte_capacity: 1024,
    }
}
