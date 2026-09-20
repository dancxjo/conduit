use conduit_core::{
    kind_id, mandatory_sign_storage_requirement, seal_plan, ArtifactId, AuthorityGrantId, BootId,
    CancellationPolicy, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId,
    ExecutionProfileId, ExpandedFormId, ExpectedSign, ExpectedTerminal, FormIdentity, FragmentId,
    FrontStartupParameter, GearId, HostId, ImplementationId, KindContractRevision, PlacementId,
    PlanFragment, PlanId, PlannedGear, PlannedSharedPool, PlanningRequestAuthority,
    PlayUnsatisfiedReason, PoolDeclarationId, PoolMemberLimits, PoolOperationId,
    PoolRealizationEnvelope, PoolRealizationHealth, PoolRealizationObservation,
    PoolSelectionDisposition, PoolSelectionEvidence, PoolSelectionEvidenceError, PortDescriptor,
    PortDirection, PortTemporal, SharedPoolId, SignId, SignStorageBudget, SourceDocumentId,
    TerminalPolicy,
};

fn member_offer(kind: &str, revision: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![FrontStartupParameter {
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
        member_front: member_offer("chat/peer", "chat/peer@1").checked_front(),
        maximum_members: 2,
        member_limits: PoolMemberLimits {
            queue_item_capacity: 4,
            queue_byte_capacity: 1_024,
            sign_item_capacity: 16,
            sign_byte_capacity: 2_048,
        },
        member_sessions_required: false,
        realization_envelope: vec![PoolRealizationEnvelope {
            host_id: HostId::from("browser-host"),
            boot_id: BootId::from("browser-boot"),
            offer_generation: conduit_core::OfferGeneration(1),
            capability_id: CapabilityId::from("browser/peer"),
            implementation_id: ImplementationId::from("browser/peer-implementation"),
            artifact_id: ArtifactId::from("browser/peer-artifact"),
            member_capacity: 2,
            resources: vec![],
            admitted_lines: vec![],
        }],
        selection_policy:
            conduit_core::SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
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
        completion_policy: conduit_core::PlanCompletionPolicy::Live,
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
                base: None,
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
fn member_compatibility_uses_checked_front_while_envelope_identity_stays_exact() {
    let pool = pool();
    let admitted = pool.realization_envelope[0].clone();
    let renamed = member_offer("renamed/browser-peer", "renamed/browser-peer@9");
    assert!(pool.permits_realization(&admitted, &renamed.checked_front()));

    let mut changed = renamed;
    changed.outputs[0].temporal = PortTemporal::Current;
    assert!(!pool.permits_realization(&admitted, &changed.checked_front()));
    let mut other_host = admitted;
    other_host.host_id = HostId::from("other-host");
    assert!(!pool.permits_realization(&other_host, &pool.member_front));
}

#[test]
fn plan_identity_seals_pool_bound_front_envelope_authority_and_consumers() {
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

#[test]
fn pool_observation_is_current_only_for_the_exact_sealed_provider_identity() {
    let realization = pool().realization_envelope.remove(0);
    let mut observation = PoolRealizationObservation {
        host_id: realization.host_id.clone(),
        boot_id: realization.boot_id.clone(),
        offer_generation: realization.offer_generation,
        capability_id: realization.capability_id.clone(),
        implementation_id: realization.implementation_id.clone(),
        artifact_id: realization.artifact_id.clone(),
        health: PoolRealizationHealth::Ready,
        sign_id: SignId::from("sign/provider-current"),
        resources: vec![],
    };
    assert!(observation.is_current_for(&realization));
    observation.offer_generation = conduit_core::OfferGeneration(2);
    assert!(!observation.is_current_for(&realization));
}

#[test]
fn selection_evidence_names_only_one_realization_from_the_immutable_plan() {
    let identity = FormIdentity {
        source_document_id: SourceDocumentId::from("source"),
        checked_form_id: CheckedFormId::from("checked"),
        expanded_form_id: ExpandedFormId::from("expanded"),
    };
    let plan = seal_plan(identity, vec![fragment(pool())]);
    let selected = PoolSelectionEvidence {
        plan_id: plan.plan_id.clone(),
        pool_id: SharedPoolId::from("room/peers"),
        operation_id: PoolOperationId::from("request/1"),
        selected_realization: Some(0),
        observation_sign_ids: vec![SignId::from("sign/resource/1")],
        disposition: PoolSelectionDisposition::Selected,
        sign_id: SignId::from("sign/selection/1"),
    };
    assert_eq!(selected.validate(&plan), Ok(()));

    let mut outside = selected.clone();
    outside.selected_realization = Some(1);
    assert_eq!(
        outside.validate(&plan),
        Err(PoolSelectionEvidenceError::RealizationOutsideEnvelope)
    );
    let exhausted = PoolSelectionEvidence {
        selected_realization: None,
        observation_sign_ids: vec![SignId::from("sign/resource/exhausted")],
        disposition: PoolSelectionDisposition::EnvelopeExhausted,
        sign_id: SignId::from("sign/exhausted"),
        ..selected
    };
    assert_eq!(exhausted.validate(&plan), Ok(()));
    let events = exhausted
        .exhaustion_replan_events(
            &plan,
            HostId::from("browser-host"),
            BootId::from("browser-boot"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/replan-request"),
        )
        .unwrap();
    assert!(matches!(
        events[0],
        conduit_core::ControlLoopEvent::PlayBecameUnsatisfied {
            reason: PlayUnsatisfiedReason::NoAdmittedPoolRealizationReady,
            ..
        }
    ));

    let mut provider_loss = exhausted;
    provider_loss.disposition = PoolSelectionDisposition::ProviderLost;
    provider_loss.selected_realization = Some(0);
    assert_eq!(
        provider_loss.exhaustion_replan_events(
            &plan,
            HostId::from("browser-host"),
            BootId::from("browser-boot"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/must-not-request")
        ),
        Err(PoolSelectionEvidenceError::InvalidDisposition)
    );
}
