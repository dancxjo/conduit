use conduit_core::*;

pub fn fragment() -> PlanFragment {
    let value_kind = KindId::from("fixture/byte@1");
    let state = PlannedStateBoundary {
        state_id: StateId::from("retained"),
        gear_id: GearId::from("cell"),
        value_kind: value_kind.clone(),
        initial_value: vec![7],
        retained: None,
        maximum_value_bytes: 1,
        continuation: StateContinuation::ExternallyBounded,
    };
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal: PortTemporal::Value,
    };
    PlanFragment {
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source"),
        checked_form_id: CheckedFormId::from("checked"),
        expanded_form_id: ExpandedFormId::from("expanded"),
        realization_backs: vec![],
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
        placements: vec![PlannedGear {
            placement_id: PlacementId::from("placement"),
            gear_id: state.gear_id.clone(),
            kind_id: kind_id("fixture/state"),
            kind_contract_revision: KindContractRevision::from("fixture/state@1"),
            execution_profile_id: ExecutionProfileId::from("fixture/state@1"),
            configuration: vec![],
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("state"),
            implementation_id: ImplementationId::from("state@1"),
            artifact_id: ArtifactId::from("state@1"),
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
            inputs: vec![port("next", PortDirection::Input)],
            outputs: vec![port("current", PortDirection::Output)],
            host_operations: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![state],
        connections: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![PlacementId::from("placement")],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
        plan_fragments: vec![],
    }
}

pub fn seal(fragment: PlanFragment) -> Plan {
    seal_plan(
        FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        },
        vec![fragment],
    )
}

#[allow(dead_code)]
pub fn retained_fragment() -> PlanFragment {
    let mut destination = fragment();
    let source = seal(fragment());
    destination.states[0].retained = Some(RetainedStateProvenance {
        source_form: FormIdentity {
            source_document_id: source.source_document_id.clone(),
            checked_form_id: source.checked_form_id.clone(),
            expanded_form_id: source.expanded_form_id.clone(),
        },
        source_play: bind_active_play(
            &source.plan_id,
            &source.fragments[0].host_id,
            &source.fragments[0].boot_id,
            3,
        ),
        source_state: destination.states[0].state_id.clone(),
        value_kind: destination.states[0].value_kind.clone(),
        generation: 17,
        current_value: vec![9],
    });
    destination
}
