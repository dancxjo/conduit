use alloc::{format, vec};
use conduit_core::{
    kind_id, mandatory_sign_storage_requirement, seal_plan, ArtifactId, AuthorityGrantId, BootId,
    CancellationPolicy, CapabilityId, CapabilityLimits, CheckedFace, CheckedFormId,
    ExecutionProfileId, ExpandedFormId, ExpectedSign, ExpectedTerminal, FormIdentity, FragmentId,
    GearId, HostId, ImplementationId, KindContractRevision, OfferGeneration, PlacementId,
    PlanFragment, PlanId, PlannedGear, PlannedSharedPool, PlanningRequestAuthority,
    PlayUnsatisfiedReason, PoolDeclarationId, PoolMemberLimits, PoolOperationId,
    PoolRealizationEnvelope, PoolRealizationHealth, PoolRealizationObservation,
    PoolSelectionDisposition, SharedPoolId, SignId, SignStorageBudget, SourceDocumentId,
    TerminalPolicy,
};
use conduit_kernel::{shared_pool::MemberKey, NodeId};

use super::*;

fn realization(index: u8) -> PoolRealizationEnvelope {
    PoolRealizationEnvelope {
        host_id: HostId::from(format!("worker-{index}")),
        boot_id: BootId::from(format!("worker-{index}/boot-1")),
        offer_generation: OfferGeneration(1),
        capability_id: CapabilityId::from(format!("worker-{index}/generate")),
        implementation_id: ImplementationId::from(format!("worker-{index}/ollama@1")),
        artifact_id: ArtifactId::from(format!("worker-{index}/model@sha256")),
        member_capacity: 1,
        resources: vec![],
    }
}

fn pool() -> PlannedSharedPool {
    PlannedSharedPool {
        pool_id: SharedPoolId::from("model/workers"),
        declaration_id: PoolDeclarationId::from("model/pool/workers"),
        member_front: CheckedFace::new(vec![], vec![], vec![], None),
        maximum_members: 2,
        member_limits: PoolMemberLimits {
            queue_item_capacity: 1,
            queue_byte_capacity: 4_096,
            sign_item_capacity: 16,
            sign_byte_capacity: 2_048,
        },
        realization_envelope: vec![realization(0), realization(1)],
        selection_policy:
            conduit_core::SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
        admission_authority: AuthorityGrantId::from("grant/model-workers"),
        consumers: vec![PlacementId::from("model-consumer")],
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
        source_document_id: SourceDocumentId::from("source/model-pool"),
        checked_form_id: CheckedFormId::from("checked/model-pool"),
        expanded_form_id: ExpandedFormId::from("expanded/model-pool"),
        completion_policy: conduit_core::PlanCompletionPolicy::Live,
        realization_backs: vec![],
        host_id: HostId::from("coordinator"),
        boot_id: BootId::from("coordinator/boot-1"),
        offer_generation: OfferGeneration(1),
        placements: vec![PlannedGear {
            placement_id: PlacementId::from("model-consumer"),
            gear_id: GearId::from("model-consumer"),
            kind_id: kind_id("test/model-consumer"),
            kind_contract_revision: KindContractRevision::from("test/model-consumer@1"),
            execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
            configuration: vec![],
            host_id: HostId::from("coordinator"),
            boot_id: BootId::from("coordinator/boot-1"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("coordinator/model-consumer"),
            implementation_id: ImplementationId::from("coordinator/model-consumer@1"),
            artifact_id: ArtifactId::from("coordinator/image@1"),
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 4_096,
            },
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![pool.pool_id.clone()],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
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

fn plan() -> conduit_core::Plan {
    seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source/model-pool"),
            checked_form_id: CheckedFormId::from("checked/model-pool"),
            expanded_form_id: ExpandedFormId::from("expanded/model-pool"),
        },
        vec![fragment(pool())],
    )
}

fn observation(
    index: u8,
    health: PoolRealizationHealth,
    sequence: u8,
) -> PoolRealizationObservation {
    let realization = realization(index);
    PoolRealizationObservation {
        host_id: realization.host_id,
        boot_id: realization.boot_id,
        offer_generation: realization.offer_generation,
        capability_id: realization.capability_id,
        implementation_id: realization.implementation_id,
        artifact_id: realization.artifact_id,
        health,
        sign_id: SignId::from(format!("sign/worker-{index}/{sequence}")),
        resources: vec![],
    }
}

fn key(value: u8) -> MemberKey {
    MemberKey([value; 32])
}

#[test]
fn hosted_controller_preserves_capacity_loss_and_replan_boundaries() {
    let plan = plan();
    let mut pool = HostedSharedPool::<2, 32, 8>::new(
        &plan,
        0,
        SharedPoolId::from("model/workers"),
        NodeId(10),
        1,
    )
    .unwrap();
    let ready = [
        observation(0, PoolRealizationHealth::Ready, 1),
        observation(1, PoolRealizationHealth::Ready, 1),
    ];
    let SharedPoolAdmissionOutcome::Selected {
        member: first,
        evidence: first_evidence,
    } = pool
        .admit_new_operation(
            PoolOperationId::from("request/1"),
            key(1),
            &ready,
            SignId::from("sign/select/1"),
        )
        .unwrap()
    else {
        panic!("first operation was refused")
    };
    assert_eq!(first.placement.realization, 0);
    assert_eq!(
        first_evidence.disposition,
        PoolSelectionDisposition::Selected
    );
    pool.trigger(first).unwrap();

    let SharedPoolAdmissionOutcome::Selected { member: second, .. } = pool
        .admit_new_operation(
            PoolOperationId::from("request/2"),
            key(2),
            &ready,
            SignId::from("sign/select/2"),
        )
        .unwrap()
    else {
        panic!("second operation was refused")
    };
    assert_eq!(second.placement.realization, 1);
    pool.trigger(second).unwrap();

    let SharedPoolAdmissionOutcome::Refused(capacity) = pool
        .admit_new_operation(
            PoolOperationId::from("request/3"),
            key(3),
            &ready,
            SignId::from("sign/select/3"),
        )
        .unwrap()
    else {
        panic!("full pool admitted a third operation")
    };
    assert_eq!(
        capacity.disposition,
        PoolSelectionDisposition::CapacityRefused
    );
    assert!(capacity
        .exhaustion_replan_events(
            &plan,
            HostId::from("coordinator"),
            BootId::from("coordinator/boot-1"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/must-not-replan")
        )
        .is_err());

    let lost = [
        observation(0, PoolRealizationHealth::Unavailable, 2),
        observation(1, PoolRealizationHealth::Ready, 2),
    ];
    let loss = pool
        .provider_lost(
            PoolOperationId::from("request/1"),
            first,
            &lost,
            SignId::from("sign/provider-lost/1"),
        )
        .unwrap();
    assert_eq!(loss.disposition, PoolSelectionDisposition::ProviderLost);
    assert!(loss
        .exhaustion_replan_events(
            &plan,
            HostId::from("coordinator"),
            BootId::from("coordinator/boot-1"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/must-not-replay")
        )
        .is_err());

    pool.release(second).unwrap();
    let SharedPoolAdmissionOutcome::Selected { member: later, .. } = pool
        .admit_new_operation(
            PoolOperationId::from("request/4"),
            key(4),
            &lost,
            SignId::from("sign/select/4"),
        )
        .unwrap()
    else {
        panic!("sealed fallback was refused")
    };
    assert_eq!(later.placement.realization, 1);

    let unavailable = [
        observation(0, PoolRealizationHealth::Unavailable, 3),
        observation(1, PoolRealizationHealth::Unavailable, 3),
    ];
    let SharedPoolAdmissionOutcome::Refused(exhausted) = pool
        .admit_new_operation(
            PoolOperationId::from("request/5"),
            key(5),
            &unavailable,
            SignId::from("sign/exhausted/5"),
        )
        .unwrap()
    else {
        panic!("unavailable envelope admitted work")
    };
    assert_eq!(
        exhausted.disposition,
        PoolSelectionDisposition::EnvelopeExhausted
    );
    let events = exhausted
        .exhaustion_replan_events(
            &plan,
            HostId::from("coordinator"),
            BootId::from("coordinator/boot-1"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/replan/5"),
        )
        .unwrap();
    assert!(matches!(
        events[0],
        conduit_core::ControlLoopEvent::PlayBecameUnsatisfied {
            reason: PlayUnsatisfiedReason::NoAdmittedPoolRealizationReady,
            ..
        }
    ));
}
