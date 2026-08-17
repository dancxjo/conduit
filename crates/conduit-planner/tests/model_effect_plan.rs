use conduit_ai::{
    EffectAuthority, EffectAuthorityDerivationError, ModelEffectProposal, ProposalDecisionOutcome,
    ProposalGate, ProviderFunctionCall, LLM_PROPOSE_KIND,
};
use conduit_core::*;
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, HostOperationBinding, HostOperationId, NodeId,
    OperationAction, RequestId, ValueRef,
};

const EFFECT_KIND: &str = "effect/send-message@1";
const ARGUMENT_KIND: &str = "llm/proposal-result@1";
const EFFECT_OPERATION: &str = "conduit.host/send-message@1";

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

fn wired_plan() -> Plan {
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
            startup_dependencies: vec![],
            startup_order: vec![
                PlacementId::from("placement/proposer"),
                PlacementId::from("placement/effect"),
            ],
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

fn proposal(plan: &Plan) -> ModelEffectProposal {
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

fn reseal(plan: Plan) -> Plan {
    seal_plan(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        plan.fragments,
    )
}

#[test]
fn exact_plan_wiring_derives_authority_and_reaches_kernel_admission() {
    let plan = wired_plan();
    assert!(verify_plan(&plan));
    let authority = EffectAuthority::from_plan(
        &plan,
        &PlacementId::from("placement/proposer"),
        &kind_id(EFFECT_KIND),
    )
    .unwrap();
    assert_eq!(authority.authority_id, "grant/send-message");
    assert_eq!(authority.maximum_argument_bytes, 512);

    let mut gate = ProposalGate::new(Some(authority), 2).unwrap();
    let disposition = gate.submit(proposal(&plan)).unwrap();
    assert!(matches!(
        disposition.decision.outcome,
        ProposalDecisionOutcome::Authorized { .. }
    ));
    let request = disposition.request.unwrap();

    let byte_len = u32::try_from(request.canonical_arguments.len()).unwrap();
    let input = BoundedValueRef::new(
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len,
        },
        512,
    )
    .unwrap();
    let mut bindings = FixedHostOperationBindings::<1>::new(1);
    bindings
        .install(
            NodeId(0),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 512,
                maximum_output_bytes: 512,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    assert!(bindings
        .admit(
            NodeId(0),
            OperationAction::RequestHostOperation {
                request: RequestId(1),
                operation: HostOperationId(0),
                input,
            },
        )
        .is_ok());
}

#[test]
fn missing_or_mutated_plan_truth_cannot_be_replaced_by_model_output() {
    let plan = wired_plan();
    let proposer = PlacementId::from("placement/proposer");
    let operation = kind_id(EFFECT_KIND);

    let mut unwired = plan.clone();
    unwired.fragments[0].connections.clear();
    let unwired = reseal(unwired);
    assert_eq!(
        EffectAuthority::from_plan(&unwired, &proposer, &operation),
        Err(EffectAuthorityDerivationError::EffectWiringMissing)
    );

    let mut invented_source_port = plan.clone();
    invented_source_port.fragments[0].connections[0].source_port_id = port_id("invented");
    let invented_source_port = reseal(invented_source_port);
    assert_eq!(
        EffectAuthority::from_plan(&invented_source_port, &proposer, &operation),
        Err(EffectAuthorityDerivationError::EffectWiringMissing)
    );

    let mut unavailable = plan.clone();
    unavailable.fragments[0].placements[1].authority.clear();
    let unavailable = reseal(unavailable);
    assert_eq!(
        EffectAuthority::from_plan(&unavailable, &proposer, &operation),
        Err(EffectAuthorityDerivationError::AuthorityMissing)
    );

    let mut wrong_subject = plan.clone();
    wrong_subject.fragments[0].placements[1].authority[0].subject_kind = kind_id("effect/other@1");
    let wrong_subject = reseal(wrong_subject);
    assert_eq!(
        EffectAuthority::from_plan(&wrong_subject, &proposer, &operation),
        Err(EffectAuthorityDerivationError::AuthorityMissing)
    );

    let mut corrupted = plan.clone();
    corrupted.plan_id = PlanId::from("model-supplied-plan");
    assert_eq!(
        EffectAuthority::from_plan(&corrupted, &proposer, &operation),
        Err(EffectAuthorityDerivationError::InvalidPlan)
    );

    let mut no_authority_gate = ProposalGate::new(None, 2).unwrap();
    assert!(no_authority_gate
        .submit(proposal(&plan))
        .unwrap()
        .request
        .is_none());
}
