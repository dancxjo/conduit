use conduit_core::{
    kind_id, mandatory_sign_storage_requirement, seal_plan, ArtifactId, AuthorityGrantId, BootId,
    CancellationPolicy, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId,
    ExecutionProfileId, ExpandedFormId, ExpectedSign, ExpectedTerminal, FaceStartupParameter,
    FormIdentity, FragmentId, GearId, HostId, ImplementationId, KindContractRevision, PlacementId,
    PlanFragment, PlanId, PlannedGear, PlannedSharedPool, PoolDeclarationId, PoolMemberLimits,
    PoolRealizationEnvelope, PortDescriptor, PortDirection, PortTemporal, SharedPoolId,
    SignStorageBudget, SourceDocumentId, TerminalPolicy,
};

fn member_offer(kind: &str, revision: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "peer".into(),
            value_type: "PeerId".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/peer"),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/peer-hosted@1"),
            implementation_id: ImplementationId::from("browser/peer-implementation"),
            artifact_id: ArtifactId::from("browser/peer-artifact"),
        },
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("recv"),
            value_kind: kind_id("ChatMessage"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("send"),
            value_kind: kind_id("ChatMessage"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
        }],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 4,
            max_queue_bytes: 1_024,
        },
    }
}

fn pool() -> PlannedSharedPool {
    PlannedSharedPool {
        pool_id: SharedPoolId::from("room/peers"),
        declaration_id: PoolDeclarationId::from("webchat/pool/peers"),
        member_face: member_offer("chat/peer", "chat/peer@1").checked_face(),
        maximum_members: 2,
        member_limits: PoolMemberLimits {
            queue_item_capacity: 4,
            queue_byte_capacity: 1_024,
            sign_item_capacity: 16,
            sign_byte_capacity: 2_048,
        },
        realization_envelope: vec![PoolRealizationEnvelope {
            host_id: HostId::from("browser-host"),
            boot_id: BootId::from("browser-boot"),
            capability_id: CapabilityId::from("browser/peer"),
            member_capacity: 2,
            resources: vec![],
        }],
        admission_authority: AuthorityGrantId::from("grant/admit-room-peer"),
        consumers: vec![PlacementId::from("room"), PlacementId::from("peer-router")],
    }
}

fn fragment(pool: PlannedSharedPool) -> PlanFragment {
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    PlanFragment {
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source"),
        checked_form_id: CheckedFormId::from("checked"),
        expanded_form_id: ExpandedFormId::from("expanded"),
        realization_backs: Vec::new(),
        host_id: HostId::from("browser-host"),
        boot_id: BootId::from("browser-boot"),
        offer_generation: conduit_core::OfferGeneration(1),
        placements: pool
            .consumers
            .iter()
            .map(|placement_id| PlannedGear {
                placement_id: placement_id.clone(),
                gear_id: GearId::from(placement_id.as_str()),
                kind_id: kind_id("test/pool-consumer"),
                kind_contract_revision: KindContractRevision::from("test/pool-consumer@1"),
                execution_profile_id: ExecutionProfileId::from("test/pool-consumer-hosted@1"),
                configuration: Vec::new(),
                host_id: HostId::from("browser-host"),
                boot_id: BootId::from("browser-boot"),
                offer_generation: conduit_core::OfferGeneration(1),
                capability_id: CapabilityId::from("browser/pool-consumer"),
                implementation_id: ImplementationId::from("browser/pool-consumer"),
                artifact_id: ArtifactId::from("browser/pool-consumer"),
                realization_characteristics: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: 1,
                },
                inputs: Vec::new(),
                outputs: Vec::new(),
                host_operations: Vec::new(),
                resources: Vec::new(),
                authority: Vec::new(),
                pool_references: vec![pool.pool_id.clone()],
            })
            .collect(),
        execution_regions: vec![],
        execution_fusions: vec![],
        states: Vec::new(),
        connections: vec![],
        shared_pools: vec![pool],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![ExpectedTerminal::PlanCompleted],
        expected_sign: expected_sign.clone(),
        sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap_or(
            SignStorageBudget {
                item_capacity: 0,
                byte_capacity: 0,
            },
        ),
        plan_fragments: vec![],
    }
}

#[test]
fn member_compatibility_uses_checked_face_while_envelope_identity_stays_exact() {
    let pool = pool();
    let renamed = member_offer("renamed/browser-peer", "renamed/browser-peer@9");
    assert!(pool.permits_realization(
        &HostId::from("browser-host"),
        &BootId::from("browser-boot"),
        &CapabilityId::from("browser/peer"),
        &renamed.checked_face(),
    ));

    let mut changed = renamed;
    changed.outputs[0].temporal = PortTemporal::Current;
    assert!(!pool.permits_realization(
        &HostId::from("browser-host"),
        &BootId::from("browser-boot"),
        &CapabilityId::from("browser/peer"),
        &changed.checked_face(),
    ));
    assert!(!pool.permits_realization(
        &HostId::from("other-host"),
        &BootId::from("browser-boot"),
        &CapabilityId::from("browser/peer"),
        &pool.member_face,
    ));
}

#[test]
fn plan_identity_seals_pool_bound_face_envelope_authority_and_consumers() {
    let identity = FormIdentity {
        source_document_id: SourceDocumentId::from("source"),
        checked_form_id: CheckedFormId::from("checked"),
        expanded_form_id: ExpandedFormId::from("expanded"),
    };
    let baseline = seal_plan(identity.clone(), vec![fragment(pool())]);
    assert!(conduit_core::verify_plan(&baseline));

    let mut changed_pool = pool();
    changed_pool.maximum_members = 1;
    let changed = seal_plan(identity, vec![fragment(changed_pool)]);
    assert!(conduit_core::verify_plan(&changed));
    assert_ne!(baseline.plan_id, changed.plan_id);

    let mut mutated = baseline;
    mutated.fragments[0].shared_pools[0].maximum_members = 1;
    assert!(!conduit_core::verify_plan(&mutated));
}
