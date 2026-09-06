use conduit_ai::{ModelEffectProposal, ProviderFunctionCall, LLM_PROPOSE_KIND};
use conduit_core::*;

pub const EFFECT_KIND: &str = "effect/send-message@1";
pub const ARGUMENT_KIND: &str = "llm/proposal-result@1";
pub const EFFECT_OPERATION: &str = PRESENT_HOST_OPERATION_CONTRACT;

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn placement(
    id: &str,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> PlannedGear {
    PlannedGear {
        placement_id: PlacementId::from(id),
        gear_id: GearId::from(id),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("conduit.{kind}@1")),
        execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
        configuration: vec![],
        host_id: HostId::from("host/a"),
        boot_id: BootId::from("boot/a"),
        offer_generation: OfferGeneration(1),
        capability_id: CapabilityId::from(format!("capability/{id}")),
        implementation_id: ImplementationId::from(format!("implementation/{id}")),
        artifact_id: ArtifactId::from(format!("artifact/{id}")),
        realization_characteristics: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 1_024,
        },
        inputs,
        outputs,
        host_operations: vec![],
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}

pub fn wired_plan() -> Plan {
    let proposer_id = PlacementId::from("placement/proposer");
    let effect_id = PlacementId::from("placement/effect");
    let mut proposer = placement(
        proposer_id.as_str(),
        LLM_PROPOSE_KIND,
        vec![port(
            "request",
            "llm/proposal-request@1",
            PortDirection::Input,
        )],
        vec![port("result", ARGUMENT_KIND, PortDirection::Output)],
    );
    proposer.host_operations.push(HostOperationRequirement {
        contract_id: HostOperationContractId::from("conduit.host/local-model-inference@1"),
        target_kind: Some(kind_id(LLM_PROPOSE_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: 1_024,
        maximum_output_bytes: 1_024,
    });

    let mut effect = placement(
        effect_id.as_str(),
        EFFECT_KIND,
        vec![port("request", ARGUMENT_KIND, PortDirection::Input)],
        vec![port(
            "result",
            "effect/message-result@1",
            PortDirection::Output,
        )],
    );
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(EFFECT_OPERATION),
        target_kind: Some(kind_id(EFFECT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: 512,
        maximum_output_bytes: 512,
    };
    effect.host_operations.push(operation.clone());
    effect.authority.push(AuthorityBinding {
        grant_id: AuthorityGrantId::from("grant/send-message"),
        contract_id: AuthorityContractId::from("authority/send-message@1"),
        host_operation_contract_id: operation.contract_id,
        subject_kind: kind_id(EFFECT_KIND),
        host_id: effect.host_id.clone(),
        boot_id: effect.boot_id.clone(),
        capability_id: effect.capability_id.clone(),
    });
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared(PlacementId::from("placement/proposer")),
        ExpectedSign::PlacementPrepared(PlacementId::from("placement/effect")),
        ExpectedSign::PlacementTerminal(PlacementId::from("placement/proposer")),
        ExpectedSign::PlacementTerminal(PlacementId::from("placement/effect")),
        ExpectedSign::ConnectionTerminal(ConnectionId::from("connection/proposal-effect")),
        ExpectedSign::PlanTerminal,
    ];
    let sign_storage_budget = mandatory_sign_storage_requirement(&expected_sign).unwrap();

    seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source/effect-demo"),
            checked_form_id: CheckedFormId::from("checked/effect-demo"),
            expanded_form_id: ExpandedFormId::from("expanded/effect-demo"),
        },
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
            placements: vec![proposer, effect],
            execution_regions: vec![],
            execution_fusions: vec![],
            states: Vec::new(),
            connections: vec![PlannedConnection {
                connection_id: ConnectionId::from("connection/proposal-effect"),
                source_placement_id: proposer_id,
                source_port_id: port_id("result"),
                sink_placement_id: effect_id,
                sink_port_id: port_id("request"),
                value_kind: kind_id(ARGUMENT_KIND),
                temporal: PortTemporal::Value,
                selected_line: None,
                admitted_lines: vec![],
                item_capacity: 1,
                byte_capacity: 512,
            }],
            shared_pools: vec![],
            startup_dependencies: vec![StartupDependency {
                prerequisite_placement_id: PlacementId::from("placement/effect"),
                dependent_placement_id: PlacementId::from("placement/proposer"),
            }],
            startup_order: vec![
                PlacementId::from("placement/effect"),
                PlacementId::from("placement/proposer"),
            ],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![
                ExpectedTerminal::PlacementCompleted(PlacementId::from("placement/proposer")),
                ExpectedTerminal::PlacementCompleted(PlacementId::from("placement/effect")),
                ExpectedTerminal::ConnectionCompleted(ConnectionId::from(
                    "connection/proposal-effect",
                )),
                ExpectedTerminal::PlanCompleted,
            ],
            expected_sign,
            sign_storage_budget,
            plan_fragments: vec![],
        }],
    )
}

pub fn proposal(plan: &Plan) -> ModelEffectProposal {
    let value_type = StructuredInfoType::leaf(kind_id(ARGUMENT_KIND)).unwrap();
    let arguments = StructuredInfoValue::leaf(value_type, b"hello".to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    ModelEffectProposal::from_provider_call(
        "proposal/model/plan-derived".into(),
        plan.plan_id.clone(),
        ProviderFunctionCall {
            function_name: EFFECT_KIND.into(),
            canonical_arguments: arguments,
        },
        "high confidence does not create authority".into(),
        vec![],
    )
}

#[allow(dead_code)] // The runtime integration shares this fixture without mutation cases.
pub fn reseal(plan: Plan) -> Plan {
    seal_plan(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        plan.fragments,
    )
}
