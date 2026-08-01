use conduit_core::{
    ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime, BlockingFairness,
    BoundednessProfile, CancellationCheckpointPolicy, CancellationGuarantee,
    CheckpointProviderCapabilities, ClockRounding, DeadlineContract, DelegationPolicy,
    DeliveryClaim, DescriptorRef, Direction, DuplicatePolicy, DuplicationRule,
    EFFECT_COMMIT_PROFILE_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION, EffectCommitProfile,
    EffectDiscontinuity, EffectIdempotency, EffectRequirement, EventClass,
    EventProviderCapabilities, EventStreamContract, ExecutionLimits, ExecutionPlan,
    ExecutionProfile, ExplicitSatisfactionRequirement, FanOutMode, FeedbackBoundaryKind,
    FeedbackInitialization, FeedbackReplayGapPolicy, FeedbackTerminalPolicy, FlowCapacity,
    FlowPolicy, FlowWatermarks, ForeignRetention, GrantStatus, HostCapability, Id, InstancePath,
    JobContract, MergeOrdering, MergeTerminalPolicy, ObservedGrant, PinnedDescriptor, PlanArtifact,
    PlanAuthority, PlanClockConversion, PlanCollection, PlanCompositeMapping, PlanDiagnosticCode,
    PlanEventStream, PlanEvidenceProviderBinding, PlanExportBinding, PlanFanOut,
    PlanFeedbackBoundary, PlanHostObservation, PlanJob, PlanMerge, PlanMergeInput, PlanPortGroup,
    PlanPortGroupMember, PlanResourceBinding, PlanResourceBudget, PlanSatisfactionProof,
    PlanSatisfactionSubject, PlanValidationContext, PlanWorkload, Pressure,
    RESOURCE_LEASE_SCHEMA_VERSION, RUNTIME_EVIDENCE_POLICY_VERSION, ReplayDelivery,
    ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, ResourceLeaseContract,
    ResourceLeaseReason, ResourceRef, ResourceSelector, ResourceSharingMode, RestartPolicy,
    RetentionPolicy, RuntimeEvidenceMode, RuntimeEvidencePolicy, SatisfactionFacet,
    SatisfactionMethod, SatisfactionObligation, SatisfactionPin, SatisfactionProof,
    SatisfactionReason, SatisfactionRole, SemanticHash, Sensitivity, StopPolicy,
    SubscriberCoupling, TypeContractRef, UnknownCommitPolicy, UnresolvedPlanConstraint,
    UnresolvedPlanKind, ValueEnvelopePolicy, WORKLOAD_CONTRACT_SCHEMA_VERSION, WatchAdmission,
    WatchRetention, WatchSubject, WorkloadBudget, WorkloadCapability, WorkloadContract,
    WorkloadEvidenceKind, WorkloadGuarantee, WorkloadLimit, resolve_authority,
    validate_execution_plan,
};

const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([8; 32]),
};
const RESOURCE: ResourceRef<'static> = ResourceRef {
    kind: Id("fixture/device"),
    id: Id("fixture/device-a"),
};
const NODE_ALLOCATION: PlanResourceBudget = PlanResourceBudget {
    memory_bytes: 100,
    storage_bytes: 0,
    cpu_units: 1,
    timers: 1,
    transports: 0,
    checkpoints: 0,
    evidence_bytes: 0,
};
const PLAN_BUDGET: PlanResourceBudget = PlanResourceBudget {
    memory_bytes: 512,
    storage_bytes: 64,
    cpu_units: 4,
    timers: 4,
    transports: 2,
    checkpoints: 2,
    evidence_bytes: 64,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn time(tick: u64) -> AuthorityTime<'static> {
    AuthorityTime {
        basis: Id("clock/monotonic"),
        tick,
    }
}

fn with_plan(test: impl FnOnce(ExecutionPlan<'_>, &mut [SemanticHash; 64])) {
    let effect = EffectRequirement {
        id: Id("read"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("fixture/read"),
        resource: ResourceSelector::Exact(RESOURCE),
        requester: InstancePath::new("root/source").unwrap(),
        audience: Id("fixture/run"),
        constraints: &[],
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id("fixture/capability"),
        action: effect.action,
        resource: RESOURCE,
        host: Id("host/a"),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 100,
    };
    let grant = AuthorityGrant {
        id: Id("fixture/grant"),
        action: effect.action,
        resource: RESOURCE,
        scope: AuthorityScope {
            root: effect.requester,
            descendants: false,
        },
        audience: effect.audience,
        constraints: &[],
        time_basis: Id("clock/monotonic"),
        not_before_tick: 0,
        expires_at_tick: 80,
        issued_for_host: Id("host/a"),
        delegation: DelegationPolicy::None,
        audit_id: Id("fixture/audit"),
        terminal_policy: StopPolicy::Abort,
    };
    let binding = resolve_authority(
        effect,
        Id("host/a"),
        time(10),
        &[capability],
        &[ObservedGrant {
            grant,
            status: GrantStatus::Active,
        }],
    )
    .unwrap();
    let effect_hash = effect.semantic_hash().unwrap();
    let required_effects = [effect_hash];
    let required_resources = [Id("fixture/source-device")];
    let lease = ResourceLeaseContract {
        schema_version: RESOURCE_LEASE_SCHEMA_VERSION,
        id: Id("fixture/source-lease"),
        resource_binding: required_resources[0],
        holder: effect.requester,
        run: Id("fixture/run"),
        epoch: 1,
        scope: Id("fixture/read-scope"),
        sharing: ResourceSharingMode::Exclusive,
        reservation: PlanResourceBudget {
            memory_bytes: 50,
            ..PlanResourceBudget::ZERO
        },
        time_basis: Id("clock/monotonic"),
        issued_at_tick: 5,
        expires_at_tick: 60,
        revocation_grace_ticks: 5,
        cleanup_ticks: 10,
        maximum_operations: 2,
        maximum_evidence_events: 4,
        cleanup_escalation: pin("fixture/force-close", 30),
        foreign_retention: ForeignRetention::Unsupported,
    };
    let commit_profile = EffectCommitProfile {
        schema_version: EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
        id: Id("fixture/read-commit"),
        operation: effect.action,
        resource_lease: lease.id,
        commit_boundary: pin("fixture/read-commit-boundary", 31),
        idempotency: EffectIdempotency::ReconcileBeforeRetry,
        unknown_commit: UnknownCommitPolicy::Reconcile,
        discontinuity: EffectDiscontinuity::ReconcileRequired,
        cleanup: pin("fixture/read-cleanup", 32),
        maximum_attempts: 2,
        evidence_events_per_attempt: 2,
    };
    let authority = PlanAuthority {
        node: effect.requester,
        effect_hash,
        grant_hash: grant.semantic_hash().unwrap(),
        effect,
        capability,
        grant,
        binding,
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
        commit_profile: Some(commit_profile),
    };
    let observations = [PlanHostObservation {
        id: Id("fixture/host-report"),
        host: Id("host/a"),
        semantic_hash: hash(10),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 100,
    }];
    let artifacts = [
        PlanArtifact {
            id: Id("fixture/source-artifact"),
            digest: ArtifactDigest::from_bytes([11; 32]),
        },
        PlanArtifact {
            id: Id("fixture/sink-artifact"),
            digest: ArtifactDigest::from_bytes([12; 32]),
        },
    ];
    let resources = [PlanResourceBinding {
        id: required_resources[0],
        node: effect.requester,
        resource: RESOURCE,
        host_observation: observations[0].id,
        lease: Some(lease),
    }];
    let mut execution_profile = ExecutionProfile {
        id: Id("fixture/current-profile"),
        schema_version: 0,
        semantic_hash: ZERO_HASH,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: ExecutionLimits {
            max_step_work: 8,
            max_retained_values: 0,
            max_retained_bytes: 0,
            max_scratch_bytes: 0,
            max_input_leases: 0,
            max_input_bytes: 0,
            max_output_reservations: 0,
            max_output_bytes: 0,
            max_transactions: 1,
            max_fragments_per_step: 0,
            max_pending_operations: 0,
            max_timers: 0,
            max_child_tasks: 0,
            max_host_buffer_bytes: 0,
            max_foreign_queue_items: 0,
            max_foreign_queue_bytes: 0,
            max_checkpoint_bytes: 0,
            implementation_memory_bytes: 0,
            cancellation_ticks: 1,
        },
        representations: &[],
        memory_claims: &[],
        checkpoint: None,
    };
    execution_profile.semantic_hash = execution_profile.computed_semantic_hash(&mut []).unwrap();
    let nodes = [
        ResolvedPlanNode {
            instance: InstancePath::new("root/source").unwrap(),
            contract: pin("fixture/source-contract", 13),
            implementation: pin("fixture/source-impl", 14),
            lifecycle_policy: pin("fixture/source-lifecycle", 28),
            execution_profile: Some(&execution_profile),
            artifact: artifacts[0].id,
            host_observation: observations[0].id,
            host: observations[0].host,
            allocation: NODE_ALLOCATION,
            required_resources: &required_resources,
            required_effects: &required_effects,
        },
        ResolvedPlanNode {
            instance: InstancePath::new("root/sink").unwrap(),
            contract: pin("fixture/sink-contract", 15),
            implementation: pin("fixture/sink-impl", 16),
            lifecycle_policy: pin("fixture/sink-lifecycle", 29),
            execution_profile: Some(&execution_profile),
            artifact: artifacts[1].id,
            host_observation: observations[0].id,
            host: observations[0].host,
            allocation: NODE_ALLOCATION,
            required_resources: &[],
            required_effects: &[],
        },
    ];
    let capacity = FlowCapacity::new(2, 32, 64).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, 2, capacity).unwrap(),
    )
    .unwrap();
    let cords = [ResolvedPlanCord {
        id: Id("values"),
        from: ResolvedPlanPort {
            node: nodes[0].instance,
            port: Id("value"),
            direction: Direction::Output,
            port_contract_hash: hash(17),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: nodes[1].instance,
            port: Id("value"),
            direction: Direction::Input,
            port_contract_hash: hash(18),
            value_type: TYPE,
        },
        flow,
        queue_memory_bytes: 64,
    }];
    let members = [nodes[0].instance, nodes[1].instance];
    let exports = [
        PlanExportBinding {
            boundary_port: Id("value"),
            member: nodes[0].instance,
            member_port: Id("value"),
            direction: Direction::Output,
        },
        PlanExportBinding {
            boundary_port: Id("value"),
            member: nodes[1].instance,
            member_port: Id("value"),
            direction: Direction::Input,
        },
    ];
    let composites = [PlanCompositeMapping {
        instance: InstancePath::new("root").unwrap(),
        definition_hash: hash(19),
        members: &members,
        exports: &exports,
    }];
    let group_members = [
        PlanPortGroupMember {
            id: Id("lane0"),
            ordinal: 0,
            port_contract_hash: hash(20),
        },
        PlanPortGroupMember {
            id: Id("lane1"),
            ordinal: 1,
            port_contract_hash: hash(21),
        },
    ];
    let groups = [PlanPortGroup {
        instance: nodes[0].instance,
        template_hash: hash(22),
        maximum: 2,
        direction: Direction::Output,
        members: &group_members,
    }];
    let authorities = [authority];
    let mut plan = ExecutionPlan {
        schema_version: 0,
        identity: ZERO_HASH,
        source_semantic_hash: hash(1),
        resolver: pin("fixture/resolver", 2),
        resolver_policy_hash: hash(3),
        created_at: time(10),
        budget: PLAN_BUDGET,
        host_observations: &observations,
        resources: &resources,
        workloads: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &[],
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        evidence_provider: None,
        watch_admissions: &[],
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &authorities,
        hazard_closure: None,
        composites: &composites,
        port_groups: &groups,
        instance_pools: &[],
        supervisions: &[],
        unresolved: &[],
    };
    let mut scratch = [ZERO_HASH; 64];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    test(plan, &mut scratch);
}

fn context(tick: u64) -> PlanValidationContext<'static> {
    PlanValidationContext {
        supported_schema_version: 0,
        now: time(tick),
    }
}

fn workload_budget(work_units: u64) -> WorkloadBudget {
    WorkloadBudget {
        work_units: WorkloadLimit::Finite(work_units),
        tasks: WorkloadLimit::Finite(1),
        processes: WorkloadLimit::Unsupported,
        descriptors: WorkloadLimit::Finite(2),
        connections: WorkloadLimit::Finite(1),
        storage_bytes: WorkloadLimit::Unsupported,
        device_operations: WorkloadLimit::Unsupported,
        network_bytes: WorkloadLimit::Unsupported,
        callbacks: WorkloadLimit::Finite(2),
        foreign_queue_items: WorkloadLimit::Finite(1),
        transition_overlap_work_units: WorkloadLimit::Finite(20),
    }
}

#[test]
fn workload_admission_is_pinned_separately_from_observations() {
    with_plan(|plan, scratch| {
        let mut profile = ExecutionProfile {
            id: Id("fixture/workload-profile"),
            schema_version: 0,
            semantic_hash: ZERO_HASH,
            boundedness: BoundednessProfile::Hard,
            cancellation: CancellationGuarantee::Bounded,
            step_bound_enforced: true,
            limits: ExecutionLimits {
                max_step_work: 8,
                max_retained_values: 0,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_leases: 0,
                max_input_bytes: 0,
                max_output_reservations: 0,
                max_output_bytes: 0,
                max_transactions: 1,
                max_fragments_per_step: 0,
                max_pending_operations: 0,
                max_timers: 0,
                max_child_tasks: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_items: 0,
                max_foreign_queue_bytes: 0,
                max_checkpoint_bytes: 0,
                implementation_memory_bytes: 0,
                cancellation_ticks: 1,
            },
            representations: &[],
            memory_claims: &[],
            checkpoint: None,
        };
        profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
        let nodes = [
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                required_resources: &[],
                required_effects: &[],
                ..plan.nodes[0]
            },
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                required_resources: &[],
                required_effects: &[],
                ..plan.nodes[1]
            },
        ];
        let contract = WorkloadContract {
            schema_version: WORKLOAD_CONTRACT_SCHEMA_VERSION,
            id: Id("workload/source"),
            service: Id("service/source"),
            node: plan.nodes[0].instance,
            guarantee: WorkloadGuarantee::Hard,
            budget: workload_budget(100),
            deadline: Some(DeadlineContract {
                time_basis: plan.created_at.basis,
                relative_deadline_ticks: 20,
                maximum_jitter_ticks: 2,
            }),
            maximum_evidence_events: 4,
        };
        let capability = WorkloadCapability {
            id: Id("capability/source-deadline"),
            identity: hash(90),
            host_observation: plan.host_observations[0].id,
            evidence_kind: WorkloadEvidenceKind::ExactEnforcement,
            time_basis: plan.created_at.basis,
            observed_at_tick: 0,
            valid_until_tick: 100,
            capacity: workload_budget(200),
            maximum_deadline_ticks: 30,
            maximum_jitter_ticks: 1,
        };
        let workloads = [PlanWorkload {
            contract,
            capability,
        }];
        let validation = PlanValidationContext {
            supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            now: time(20),
        };
        let mut current = ExecutionPlan {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            identity: ZERO_HASH,
            resources: &[],
            workloads: &workloads,
            nodes: &nodes,
            authorities: &[],
            composites: &[],
            port_groups: &[],
            instance_pools: &[],
            ..plan
        };
        current.identity = current.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&current, validation, scratch),
            Ok(())
        );

        let benchmark = [PlanWorkload {
            capability: WorkloadCapability {
                evidence_kind: WorkloadEvidenceKind::Benchmark,
                ..capability
            },
            ..workloads[0]
        }];
        let mut benchmark_plan = ExecutionPlan {
            identity: ZERO_HASH,
            workloads: &benchmark,
            ..current
        };
        benchmark_plan.identity = benchmark_plan.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&benchmark_plan, validation, scratch),
            Err(conduit_core::PlanValidationError {
                code: PlanDiagnosticCode::Workload(
                    conduit_core::WorkloadReason::BenchmarkIsNotAuthority,
                ),
                collection: PlanCollection::Workloads,
                subject_index: Some(0),
            })
        );
    });
}

#[test]
fn valid_nested_plan_pins_every_runnable_boundary() {
    with_plan(|plan, scratch| {
        assert_eq!(
            plan.identity.to_string(),
            "sha256:eb57d8a56634b735fb5a70506d57a32a1244cd7303de529d951673ddd06b4c0a"
        );
        assert_eq!(validate_execution_plan(&plan, context(20), scratch), Ok(()));
        assert_eq!(plan.identity, plan.semantic_hash(scratch).unwrap());
        assert_eq!(plan.nodes[0].required_effects.len(), 1);

        let mut minimal = ExecutionPlan {
            identity: ZERO_HASH,
            composites: &[],
            port_groups: &[],
            instance_pools: &[],
            ..plan
        };
        minimal.identity = minimal.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&minimal, context(20), scratch),
            Ok(())
        );

        let fixture = include_str!("../../../conformance/c2/execution-plan.tsv");
        for case in [
            "valid_minimal",
            "valid_nested",
            "duplicate_id",
            "dangling_endpoint",
            "hash_mismatch",
            "zero_or_unbounded_capacity",
            "unresolved_implementation",
            "missing_artifact",
            "over_budget",
            "absent_grant",
            "expired_grant",
            "stale_host_report",
            "unsupported_version",
            "canonical_identity",
            "bounded_instance_pool",
        ] {
            assert!(
                fixture.lines().any(|line| line.starts_with(case)),
                "missing fixture {case}"
            );
        }
    });
}

#[test]
fn resource_lease_and_domain_commit_disposition_are_pinned() {
    with_plan(|plan, scratch| {
        let mut execution_profile = ExecutionProfile {
            id: Id("fixture/lease-execution-profile"),
            schema_version: 0,
            semantic_hash: ZERO_HASH,
            boundedness: BoundednessProfile::Hard,
            cancellation: CancellationGuarantee::Bounded,
            step_bound_enforced: true,
            limits: ExecutionLimits {
                max_step_work: 8,
                max_retained_values: 0,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_leases: 0,
                max_input_bytes: 0,
                max_output_reservations: 0,
                max_output_bytes: 0,
                max_transactions: 1,
                max_fragments_per_step: 0,
                max_pending_operations: 0,
                max_timers: 0,
                max_child_tasks: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_items: 0,
                max_foreign_queue_bytes: 0,
                max_checkpoint_bytes: 0,
                implementation_memory_bytes: 0,
                cancellation_ticks: 1,
            },
            representations: &[],
            memory_claims: &[],
            checkpoint: None,
        };
        execution_profile.semantic_hash =
            execution_profile.computed_semantic_hash(&mut []).unwrap();
        let nodes = [
            ResolvedPlanNode {
                execution_profile: Some(&execution_profile),
                ..plan.nodes[0]
            },
            ResolvedPlanNode {
                execution_profile: Some(&execution_profile),
                ..plan.nodes[1]
            },
        ];
        let lease = ResourceLeaseContract {
            schema_version: RESOURCE_LEASE_SCHEMA_VERSION,
            id: Id("fixture/source-lease"),
            resource_binding: plan.resources[0].id,
            holder: plan.resources[0].node,
            run: Id("fixture/run"),
            epoch: 7,
            scope: Id("fixture/read-scope"),
            sharing: ResourceSharingMode::Exclusive,
            reservation: PlanResourceBudget {
                memory_bytes: 50,
                ..PlanResourceBudget::ZERO
            },
            time_basis: plan.created_at.basis,
            issued_at_tick: 5,
            expires_at_tick: 60,
            revocation_grace_ticks: 5,
            cleanup_ticks: 10,
            maximum_operations: 2,
            maximum_evidence_events: 4,
            cleanup_escalation: pin("fixture/force-close", 30),
            foreign_retention: ForeignRetention::Unsupported,
        };
        let profile = EffectCommitProfile {
            schema_version: EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
            id: Id("fixture/read-commit"),
            operation: plan.authorities[0].effect.action,
            resource_lease: lease.id,
            commit_boundary: pin("fixture/read-commit-boundary", 31),
            idempotency: EffectIdempotency::ReconcileBeforeRetry,
            unknown_commit: UnknownCommitPolicy::Reconcile,
            discontinuity: EffectDiscontinuity::ReconcileRequired,
            cleanup: pin("fixture/read-cleanup", 32),
            maximum_attempts: 2,
            evidence_events_per_attempt: 2,
        };
        let resources = [PlanResourceBinding {
            lease: Some(lease),
            ..plan.resources[0]
        }];
        let authorities = [PlanAuthority {
            commit_profile: Some(profile),
            ..plan.authorities[0]
        }];
        let mut plan = ExecutionPlan {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            resources: &resources,
            authorities: &authorities,
            nodes: &nodes,
            instance_pools: &[],
            identity: ZERO_HASH,
            ..plan
        };
        plan.identity = plan.semantic_hash(scratch).unwrap();
        let latest_context = PlanValidationContext {
            supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            now: time(20),
        };
        assert_eq!(
            validate_execution_plan(&plan, latest_context, scratch),
            Ok(())
        );

        let no_commit_authorities = [PlanAuthority {
            commit_profile: None,
            ..authorities[0]
        }];
        let mut no_commit = ExecutionPlan {
            authorities: &no_commit_authorities,
            identity: ZERO_HASH,
            ..plan
        };
        no_commit.identity = no_commit.semantic_hash(scratch).unwrap();
        let error = validate_execution_plan(&no_commit, latest_context, scratch).unwrap_err();
        assert_eq!(
            error.code,
            PlanDiagnosticCode::ResourceLease(ResourceLeaseReason::InvalidContract)
        );
        assert_eq!(error.collection, PlanCollection::Authorities);

        let wrong_holder_lease = ResourceLeaseContract {
            holder: InstancePath::new("root/sink").unwrap(),
            ..lease
        };
        let wrong_holder_resources = [PlanResourceBinding {
            lease: Some(wrong_holder_lease),
            ..resources[0]
        }];
        let mut wrong_holder = ExecutionPlan {
            resources: &wrong_holder_resources,
            identity: ZERO_HASH,
            ..plan
        };
        wrong_holder.identity = wrong_holder.semantic_hash(scratch).unwrap();
        let error = validate_execution_plan(&wrong_holder, latest_context, scratch).unwrap_err();
        assert_eq!(
            error.code,
            PlanDiagnosticCode::ResourceLease(ResourceLeaseReason::IdentityMismatch)
        );
        assert_eq!(error.collection, PlanCollection::Resources);
    });
}

#[test]
fn canonical_identity_ignores_registry_and_collection_order() {
    with_plan(|plan, scratch| {
        let artifacts = [plan.artifacts[1], plan.artifacts[0]];
        let nodes = [plan.nodes[1], plan.nodes[0]];
        let members = [plan.composites[0].members[1], plan.composites[0].members[0]];
        let composites = [PlanCompositeMapping {
            members: &members,
            ..plan.composites[0]
        }];
        let group_members = [
            plan.port_groups[0].members[1],
            plan.port_groups[0].members[0],
        ];
        let groups = [PlanPortGroup {
            members: &group_members,
            ..plan.port_groups[0]
        }];
        let reordered = ExecutionPlan {
            artifacts: &artifacts,
            nodes: &nodes,
            composites: &composites,
            port_groups: &groups,
            ..plan
        };
        assert_eq!(reordered.semantic_hash(scratch).unwrap(), plan.identity);
        assert_eq!(
            validate_execution_plan(&reordered, context(20), scratch),
            Ok(())
        );

        let changed_nodes = [
            ResolvedPlanNode {
                lifecycle_policy: pin("fixture/other-lifecycle", 77),
                ..plan.nodes[0]
            },
            plan.nodes[1],
        ];
        let changed_policy = ExecutionPlan {
            nodes: &changed_nodes,
            ..plan
        };
        assert_ne!(
            changed_policy.semantic_hash(scratch).unwrap(),
            plan.identity
        );
        assert_ne!(
            ExecutionPlan {
                source_semantic_hash: hash(78),
                ..plan
            }
            .semantic_hash(scratch)
            .unwrap(),
            plan.identity
        );
    });
}

#[test]
fn current_plan_pins_group_maximum_and_direction() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c2/port-group-correlation.json");
        for case in [
            "plan-preserved",
            "plan-maximum",
            "plan-direction",
            "plan-membership-over-maximum",
        ] {
            assert!(fixture.contains(&format!("\"id\": \"{case}\"")));
        }
        let changed_group = [PlanPortGroup {
            maximum: 99,
            direction: Direction::Input,
            ..plan.port_groups[0]
        }];
        let changed = ExecutionPlan {
            port_groups: &changed_group,
            ..plan
        };
        assert_ne!(changed.semantic_hash(scratch).unwrap(), plan.identity);

        let changed_maximum_group = [PlanPortGroup {
            maximum: 3,
            ..plan.port_groups[0]
        }];
        let changed_maximum = ExecutionPlan {
            port_groups: &changed_maximum_group,
            ..plan
        };
        assert_ne!(
            changed_maximum.semantic_hash(scratch).unwrap(),
            plan.identity
        );

        let changed_direction_group = [PlanPortGroup {
            direction: Direction::Input,
            ..plan.port_groups[0]
        }];
        let changed_direction = ExecutionPlan {
            port_groups: &changed_direction_group,
            ..plan
        };
        assert_ne!(
            changed_direction.semantic_hash(scratch).unwrap(),
            plan.identity
        );

        let invalid_group = [PlanPortGroup {
            maximum: 1,
            ..plan.port_groups[0]
        }];
        let mut invalid = ExecutionPlan {
            identity: ZERO_HASH,
            port_groups: &invalid_group,
            ..plan
        };
        invalid.identity = invalid.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&invalid, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::InvalidDescriptor
        );
        let mut displaced = ExecutionPlan {
            schema_version: 1,
            identity: ZERO_HASH,
            ..plan
        };
        displaced.identity = displaced.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&displaced, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::UnsupportedVersion
        );
    });
}

#[test]
fn current_plan_pins_bounded_execution_profiles() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c4/implementation-step.json");
        for case in [
            "plan-profile-pinned",
            "plan-identities-preserved",
            "plan-missing-profile-rejected",
            "plan-profile-rejected",
        ] {
            assert!(fixture.contains(&format!("\"id\":\"{case}\"")));
        }
        let mut profile = ExecutionProfile {
            id: Id("fixture/execution-profile"),
            schema_version: 0,
            semantic_hash: ZERO_HASH,
            boundedness: BoundednessProfile::Hard,
            cancellation: CancellationGuarantee::Bounded,
            step_bound_enforced: true,
            limits: ExecutionLimits {
                max_step_work: 8,
                max_retained_values: 0,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_leases: 0,
                max_input_bytes: 0,
                max_output_reservations: 0,
                max_output_bytes: 0,
                max_transactions: 1,
                max_fragments_per_step: 0,
                max_pending_operations: 0,
                max_timers: 0,
                max_child_tasks: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_items: 0,
                max_foreign_queue_bytes: 0,
                max_checkpoint_bytes: 0,
                implementation_memory_bytes: 0,
                cancellation_ticks: 1,
            },
            representations: &[],
            memory_claims: &[],
            checkpoint: None,
        };
        profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
        let nodes = [
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..plan.nodes[0]
            },
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..plan.nodes[1]
            },
        ];
        let mut current = ExecutionPlan {
            identity: ZERO_HASH,
            nodes: &nodes,
            ..plan
        };
        current.identity = current.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&current, context(20), scratch),
            Ok(())
        );
        assert_ne!(current.identity, plan.identity);

        let missing_profile_nodes = [
            ResolvedPlanNode {
                execution_profile: None,
                ..nodes[0]
            },
            nodes[1],
        ];
        let mut missing = ExecutionPlan {
            identity: ZERO_HASH,
            nodes: &missing_profile_nodes,
            ..current
        };
        missing.identity = missing.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&missing, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::InvalidDescriptor
        );
    });
}

#[test]
fn current_plan_pins_coupled_fanout_and_deterministic_merge() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c4/structural-flow.json");
        for case in [
            "plan-coupled-fanout-pinned",
            "plan-multi-edge-without-fanout-rejected",
            "non-copyable-value-rejected",
            "deterministic-round-robin",
            "event-time-late-value",
            "plan-identities-preserved",
        ] {
            assert!(fixture.contains(&format!("\"id\":\"{case}\"")));
        }
        let resonance = include_str!("../../../conformance/c4/resonance.json");
        for case in [
            "plan-stream-identity",
            "plan-identities-preserved",
            "durability-provider-rejected",
            "retention-provider-rejected",
            "security-provider-rejected",
        ] {
            assert!(resonance.contains(&format!("\"id\":\"{case}\"")));
        }
        let mut profile = ExecutionProfile {
            id: Id("fixture/structural-profile"),
            schema_version: 0,
            semantic_hash: ZERO_HASH,
            boundedness: BoundednessProfile::Hard,
            cancellation: CancellationGuarantee::Bounded,
            step_bound_enforced: true,
            limits: ExecutionLimits {
                max_step_work: 8,
                max_transactions: 1,
                cancellation_ticks: 1,
                max_retained_values: 0,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_leases: 0,
                max_input_bytes: 0,
                max_output_reservations: 0,
                max_output_bytes: 0,
                max_fragments_per_step: 0,
                max_pending_operations: 0,
                max_timers: 0,
                max_child_tasks: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_items: 0,
                max_foreign_queue_bytes: 0,
                max_checkpoint_bytes: 0,
                implementation_memory_bytes: 0,
            },
            representations: &[],
            memory_claims: &[],
            checkpoint: None,
        };
        profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
        let nodes = [
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..plan.nodes[0]
            },
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..plan.nodes[1]
            },
        ];
        let cords = [
            plan.cords[0],
            ResolvedPlanCord {
                id: Id("values-secondary"),
                ..plan.cords[0]
            },
        ];
        let branches = [cords[0].id, cords[1].id];
        let fanouts = [PlanFanOut {
            id: Id("fixture/fanout"),
            producer: cords[0].from,
            mode: FanOutMode::Coupled,
            branches: &branches,
            duplicator: None,
            duplicator_input: None,
            duplication: DuplicationRule::Copy(pin("fixture/copy-value", 70)),
        }];
        let merge_inputs = [
            PlanMergeInput {
                cord: cords[0].id,
                ordinal: 0,
                priority: 0,
            },
            PlanMergeInput {
                cord: cords[1].id,
                ordinal: 1,
                priority: 0,
            },
        ];
        let merges = [PlanMerge {
            id: Id("fixture/merge"),
            node: nodes[1].instance,
            inputs: &merge_inputs,
            ordering: MergeOrdering::RoundRobin,
            terminal: MergeTerminalPolicy::DrainAll,
        }];
        let mut v4 = ExecutionPlan {
            schema_version: 0,
            identity: ZERO_HASH,
            nodes: &nodes,
            cords: &cords,
            fanouts: &fanouts,
            merges: &merges,
            ..plan
        };
        v4.identity = v4.semantic_hash(scratch).unwrap();
        let context = PlanValidationContext {
            supported_schema_version: 0,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v4, context, scratch), Ok(()));

        let mut implicit = ExecutionPlan {
            identity: ZERO_HASH,
            fanouts: &[],
            ..v4
        };
        implicit.identity = implicit.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&implicit, context, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::StructuralInvalid
        );

        let invalid_share = [PlanFanOut {
            duplication: DuplicationRule::SharedHandle,
            ..fanouts[0]
        }];
        let mut non_copyable = ExecutionPlan {
            identity: ZERO_HASH,
            fanouts: &invalid_share,
            ..v4
        };
        non_copyable.identity = non_copyable.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&non_copyable, context, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::DuplicationUnauthorized
        );

        let isolated_cords = [
            cords[0],
            cords[1],
            ResolvedPlanCord {
                id: Id("duplicator-input"),
                from: ResolvedPlanPort {
                    node: nodes[1].instance,
                    port: Id("out"),
                    direction: Direction::Output,
                    ..cords[0].from
                },
                to: ResolvedPlanPort {
                    node: nodes[0].instance,
                    port: Id("in"),
                    direction: Direction::Input,
                    ..cords[0].to
                },
                ..cords[0]
            },
        ];
        let isolated_fanouts = [PlanFanOut {
            mode: FanOutMode::Isolated,
            duplicator: Some(nodes[0].instance),
            duplicator_input: Some(isolated_cords[2].id),
            ..fanouts[0]
        }];
        let mut isolated = ExecutionPlan {
            identity: ZERO_HASH,
            cords: &isolated_cords,
            fanouts: &isolated_fanouts,
            ..v4
        };
        isolated.identity = isolated.semantic_hash(scratch).unwrap();
        assert_eq!(validate_execution_plan(&isolated, context, scratch), Ok(()));
        assert_ne!(isolated.identity, v4.identity);

        let stream = PlanEventStream {
            publisher: nodes[0].instance,
            contract: EventStreamContract {
                id: Id("stream/events"),
                event_class: EventClass::Domain,
                payload_type: TYPE,
                retention: RetentionPolicy::Ring {
                    maximum_events: 2,
                    maximum_bytes: 64,
                },
                subscriber_coupling: SubscriberCoupling::Isolated(cords[0].flow),
                delivery: ReplayDelivery::AtLeastOnce,
                maximum_publishers: 1,
                maximum_subscribers: 2,
                maximum_pending_operations: 1,
                maximum_projection_bytes: 64,
                provider: pin("provider/retained", 81),
                recording_authority: None,
                sensitivity: Sensitivity::Public,
                terminal_evidence_required: true,
            },
            provider_capabilities: EventProviderCapabilities {
                ephemeral: true,
                retained: true,
                durable: false,
                checkpoint_cursor: false,
                integrity: true,
                redaction: false,
                maximum_events: 2,
                maximum_bytes: 64,
                maximum_subscribers: 2,
                maximum_pending_operations: 1,
            },
            allocation: PlanResourceBudget {
                memory_bytes: 64,
                evidence_bytes: 16,
                ..PlanResourceBudget::ZERO
            },
        };
        let mut v5 = ExecutionPlan {
            schema_version: 0,
            identity: ZERO_HASH,
            event_streams: &[stream],
            runtime_evidence: None,
            evidence_provider: None,
            ..v4
        };
        v5.identity = v5.semantic_hash(scratch).unwrap();
        let context5 = PlanValidationContext {
            supported_schema_version: 0,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v5, context5, scratch), Ok(()));
        assert_ne!(v5.identity, v4.identity);
        assert!(v4.event_streams.is_empty());
        assert_eq!(v5.event_streams[0].contract.id, Id("stream/events"));

        let mut incapable = ExecutionPlan {
            identity: ZERO_HASH,
            event_streams: &[PlanEventStream {
                provider_capabilities: EventProviderCapabilities {
                    retained: false,
                    ..stream.provider_capabilities
                },
                ..stream
            }],
            runtime_evidence: None,
            evidence_provider: None,
            ..v5
        };
        incapable.identity = incapable.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&incapable, context5, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::EventStreamInvalid
        );

        let job_stream = PlanEventStream {
            contract: EventStreamContract {
                id: Id("stream/job-evidence"),
                event_class: EventClass::NormativeEvidence,
                retention: RetentionPolicy::DurableAppend {
                    maximum_events: 32,
                    maximum_bytes: 128,
                    flush_ticks: 2,
                },
                subscriber_coupling: SubscriberCoupling::Isolated(cords[0].flow),
                delivery: ReplayDelivery::AtLeastOnce,
                maximum_publishers: 1,
                maximum_subscribers: 1,
                maximum_pending_operations: 2,
                maximum_projection_bytes: 64,
                provider: pin("provider/job-evidence", 82),
                terminal_evidence_required: true,
                ..stream.contract
            },
            provider_capabilities: EventProviderCapabilities {
                ephemeral: false,
                retained: true,
                durable: true,
                checkpoint_cursor: true,
                integrity: true,
                redaction: true,
                maximum_events: 32,
                maximum_bytes: 128,
                maximum_subscribers: 1,
                maximum_pending_operations: 2,
            },
            allocation: PlanResourceBudget {
                storage_bytes: 128,
                timers: 1,
                evidence_bytes: 16,
                ..PlanResourceBudget::ZERO
            },
            ..stream
        };
        let job = PlanJob {
            owner: nodes[0].instance,
            contract: JobContract {
                id: Id("job-contract/reference"),
                total_work_units: 10,
                maximum_attempts: 3,
                retry_backoff_ticks: 2,
                attempt_deadline_ticks: 50,
                maximum_checkpoints: 2,
                maximum_checkpoint_bytes: 32,
                maximum_checkpoint_state_refs: 4,
                maximum_checkpoint_operations: 2,
                lease_basis: Id("clock/monotonic"),
                maximum_lease_renewals: 1,
                delivery: DeliveryClaim::AtLeastOnce,
                duplicate_policy: DuplicatePolicy::ReturnCommitted,
                commit_boundary: pin("boundary/result-store", 83),
                transactional_boundary: None,
                checkpoint_provider: Some(pin("provider/checkpoints", 84)),
                evidence_stream: job_stream.contract.id,
                restart: RestartPolicy::ResumeRequired,
                cancellation_checkpoint: CancellationCheckpointPolicy::FinalCheckpoint {
                    maximum_ticks: 4,
                },
                result_validation: None,
            },
            checkpoint_provider_capabilities: Some(CheckpointProviderCapabilities {
                durable: true,
                integrity: true,
                migration: true,
                maximum_checkpoints: 2,
                maximum_checkpoint_bytes: 32,
                maximum_state_references: 4,
                maximum_pending_operations: 2,
            }),
            allocation: PlanResourceBudget {
                memory_bytes: 32,
                storage_bytes: 64,
                timers: 2,
                checkpoints: 2,
                ..PlanResourceBudget::ZERO
            },
        };
        let job_streams = [job_stream];
        let jobs = [job];
        let mut v6 = ExecutionPlan {
            schema_version: 0,
            identity: ZERO_HASH,
            budget: PlanResourceBudget {
                memory_bytes: 512,
                storage_bytes: 256,
                cpu_units: 4,
                timers: 8,
                transports: 2,
                checkpoints: 4,
                evidence_bytes: 64,
            },
            event_streams: &job_streams,
            runtime_evidence: None,
            evidence_provider: None,
            jobs: &jobs,
            ..v5
        };
        v6.identity = v6.semantic_hash(scratch).unwrap();
        let context6 = PlanValidationContext {
            supported_schema_version: 0,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v6, context6, scratch), Ok(()));
        assert_ne!(v6.identity, v5.identity);

        let alternate_type = TypeContractRef {
            contract_id: Id("fixture/alternate-value"),
            schema_version: 0,
            semantic_hash: hash(91),
        };
        let structural_cords = [
            ResolvedPlanCord {
                from: ResolvedPlanPort {
                    value_type: alternate_type,
                    ..v6.cords[0].from
                },
                ..v6.cords[0]
            },
            ResolvedPlanCord {
                from: ResolvedPlanPort {
                    value_type: alternate_type,
                    ..v6.cords[1].from
                },
                ..v6.cords[1]
            },
        ];
        let structural_fanouts = [PlanFanOut {
            producer: structural_cords[0].from,
            ..v6.fanouts[0]
        }];
        let obligation_ids = [
            "direction",
            "semantic-type",
            "presence",
            "connection-cardinality",
            "value-cardinality",
            "delivery",
            "temporal",
            "terminal",
            "sensitivity",
            "authority",
            "representation",
            "ownership-lifetime",
            "flow",
            "boundedness",
        ];
        let obligations = obligation_ids.map(|id| {
            let (required_hash, offered_hash) = if id == "semantic-type" {
                (
                    structural_cords[0].to.value_type.semantic_hash,
                    structural_cords[0].from.value_type.semantic_hash,
                )
            } else {
                (hash(92), hash(92))
            };
            SatisfactionObligation {
                id: Id(id),
                required_hash,
                offered_hash,
                outcome: conduit_core::CompatibilityOutcome::Compatible,
                reason: Id("fixture/accepted"),
            }
        });
        let facets = [SatisfactionFacet {
            id: Id("fixture/complete-port"),
            required_hash: hash(93),
            offered_hash: hash(93),
        }];
        let mut satisfaction = SatisfactionProof {
            schema_version: 0,
            identity: ZERO_HASH,
            role: SatisfactionRole::PortConnection,
            method: SatisfactionMethod::StructuralFacets,
            required: DescriptorRef {
                kind: Id("conduit/port-contract"),
                schema_version: 0,
                semantic_hash: structural_cords[0].to.port_contract_hash,
            },
            offered: DescriptorRef {
                kind: Id("conduit/port-contract"),
                schema_version: 0,
                semantic_hash: structural_cords[0].from.port_contract_hash,
            },
            provider: Some(SatisfactionPin {
                descriptor: DescriptorRef {
                    kind: Id("fixture/type-provider"),
                    schema_version: 0,
                    semantic_hash: hash(94),
                },
            }),
            provider_rule: Some(Id("fixture/structural-port")),
            policy: None,
            facets: &facets,
            obligations: &obligations,
            outcome: conduit_core::CompatibilityOutcome::Compatible,
            reason: SatisfactionReason::Satisfied,
            explanation: Id("fixture/complete-directional-proof"),
            explicit_requirement: ExplicitSatisfactionRequirement::None,
        };
        satisfaction.identity = satisfaction.semantic_hash(&mut [ZERO_HASH; 15]).unwrap();
        let implementation_obligation_ids = [
            "semantic-contract",
            "ports",
            "configuration",
            "representation",
            "ownership-lifetime",
            "lifecycle",
            "authority",
            "resources",
            "boundedness",
        ];
        let implementation_obligations =
            implementation_obligation_ids.map(|id| SatisfactionObligation {
                id: Id(id),
                required_hash: hash(95),
                offered_hash: hash(95),
                outcome: conduit_core::CompatibilityOutcome::Compatible,
                reason: Id("fixture/accepted"),
            });
        let mut implementation_proof = SatisfactionProof {
            schema_version: 0,
            identity: ZERO_HASH,
            role: SatisfactionRole::Implementation,
            method: SatisfactionMethod::ProviderRule,
            required: DescriptorRef {
                kind: v6.nodes[0].contract.id,
                schema_version: v6.nodes[0].contract.schema_version,
                semantic_hash: v6.nodes[0].contract.semantic_hash,
            },
            offered: DescriptorRef {
                kind: v6.nodes[0].implementation.id,
                schema_version: v6.nodes[0].implementation.schema_version,
                semantic_hash: v6.nodes[0].implementation.semantic_hash,
            },
            provider: Some(SatisfactionPin {
                descriptor: DescriptorRef {
                    kind: Id("fixture/implementation-provider"),
                    schema_version: 0,
                    semantic_hash: hash(96),
                },
            }),
            provider_rule: Some(Id("fixture/implementation-satisfies")),
            policy: None,
            facets: &[],
            obligations: &implementation_obligations,
            outcome: conduit_core::CompatibilityOutcome::Compatible,
            reason: SatisfactionReason::Satisfied,
            explanation: Id("fixture/implementation-proof"),
            explicit_requirement: ExplicitSatisfactionRequirement::None,
        };
        implementation_proof.identity = implementation_proof
            .semantic_hash(&mut [ZERO_HASH; 10])
            .unwrap();
        let proof_bindings = [
            PlanSatisfactionProof {
                subject: PlanSatisfactionSubject::Cord(structural_cords[0].id),
                proof: satisfaction,
            },
            PlanSatisfactionProof {
                subject: PlanSatisfactionSubject::Cord(structural_cords[1].id),
                proof: satisfaction,
            },
            PlanSatisfactionProof {
                subject: PlanSatisfactionSubject::Implementation(v6.nodes[0].instance),
                proof: implementation_proof,
            },
        ];
        let mut v7 = ExecutionPlan {
            schema_version: 0,
            identity: ZERO_HASH,
            cords: &structural_cords,
            fanouts: &structural_fanouts,
            satisfaction_proofs: &proof_bindings,
            ..v6
        };
        v7.identity = v7.semantic_hash(scratch).unwrap();
        let context7 = PlanValidationContext {
            supported_schema_version: 0,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v7, context7, scratch), Ok(()));
        assert_eq!(v7.source_semantic_hash, v6.source_semantic_hash);
        assert_ne!(v7.identity, v6.identity);

        let alternate_nodes = [
            ResolvedPlanNode {
                implementation: pin("fixture/alternate-source-impl", 97),
                ..v7.nodes[0]
            },
            v7.nodes[1],
        ];
        let mut alternate_implementation_proof = implementation_proof;
        alternate_implementation_proof.offered = DescriptorRef {
            kind: alternate_nodes[0].implementation.id,
            schema_version: alternate_nodes[0].implementation.schema_version,
            semantic_hash: alternate_nodes[0].implementation.semantic_hash,
        };
        alternate_implementation_proof.identity = alternate_implementation_proof
            .semantic_hash(&mut [ZERO_HASH; 10])
            .unwrap();
        let alternate_proof_bindings = [
            proof_bindings[0],
            proof_bindings[1],
            PlanSatisfactionProof {
                subject: PlanSatisfactionSubject::Implementation(alternate_nodes[0].instance),
                proof: alternate_implementation_proof,
            },
        ];
        let mut alternate_v7 = ExecutionPlan {
            identity: ZERO_HASH,
            nodes: &alternate_nodes,
            satisfaction_proofs: &alternate_proof_bindings,
            ..v7
        };
        alternate_v7.identity = alternate_v7.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&alternate_v7, context7, scratch),
            Ok(())
        );
        assert_eq!(alternate_v7.source_semantic_hash, v7.source_semantic_hash);
        assert_ne!(alternate_v7.identity, v7.identity);

        let mut missing_proof = ExecutionPlan {
            identity: ZERO_HASH,
            satisfaction_proofs: &[],
            ..v7
        };
        missing_proof.identity = missing_proof.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&missing_proof, context7, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::ContractMismatch
        );

        let mut wrong_type_obligations = obligations;
        wrong_type_obligations
            .iter_mut()
            .find(|obligation| obligation.id == Id("semantic-type"))
            .unwrap()
            .offered_hash = hash(98);
        let mut wrong_type_proof = satisfaction;
        wrong_type_proof.obligations = &wrong_type_obligations;
        wrong_type_proof.identity = wrong_type_proof
            .semantic_hash(&mut [ZERO_HASH; 15])
            .unwrap();
        let wrong_type_bindings = [
            PlanSatisfactionProof {
                subject: proof_bindings[0].subject,
                proof: wrong_type_proof,
            },
            proof_bindings[1],
            proof_bindings[2],
        ];
        let mut wrong_type_plan = ExecutionPlan {
            identity: ZERO_HASH,
            satisfaction_proofs: &wrong_type_bindings,
            ..v7
        };
        wrong_type_plan.identity = wrong_type_plan.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&wrong_type_plan, context7, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::SatisfactionInvalid
        );

        let runtime_policy = RuntimeEvidencePolicy {
            schema_version: RUNTIME_EVIDENCE_POLICY_VERSION,
            mode: RuntimeEvidenceMode::Record,
            stream: Some(job_stream.contract.id),
            maximum_events: 4,
            maximum_bytes: 64,
            required_reserve_events: 1,
            required_reserve_bytes: 16,
            telemetry_period: 2,
            telemetry_offset: 0,
            gap_summary_bytes: 8,
        };
        let mut v8 = ExecutionPlan {
            schema_version: 0,
            identity: ZERO_HASH,
            runtime_evidence: Some(runtime_policy),
            evidence_provider: None,
            ..v7
        };
        v8.identity = v8.semantic_hash(scratch).unwrap();
        let context8 = PlanValidationContext {
            supported_schema_version: 0,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v8, context8, scratch), Ok(()));
        assert_ne!(v8.identity, v7.identity);
        assert_eq!(v8.source_semantic_hash, v7.source_semantic_hash);

        let mut changed_evidence = ExecutionPlan {
            identity: ZERO_HASH,
            runtime_evidence: Some(RuntimeEvidencePolicy {
                telemetry_period: 3,
                ..runtime_policy
            }),
            ..v8
        };
        changed_evidence.identity = changed_evidence.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&changed_evidence, context8, scratch),
            Ok(())
        );
        assert_ne!(changed_evidence.identity, v8.identity);

        let incapable_jobs = [PlanJob {
            checkpoint_provider_capabilities: Some(CheckpointProviderCapabilities {
                durable: false,
                ..job.checkpoint_provider_capabilities.unwrap()
            }),
            ..job
        }];
        let mut incapable_job = ExecutionPlan {
            identity: ZERO_HASH,
            jobs: &incapable_jobs,
            ..v6
        };
        incapable_job.identity = incapable_job.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&incapable_job, context6, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::JobInvalid
        );

        let mut changed = ExecutionPlan {
            identity: ZERO_HASH,
            merges: &[PlanMerge {
                ordering: MergeOrdering::Arrival,
                ..merges[0]
            }],
            ..v4
        };
        changed.identity = changed.semantic_hash(scratch).unwrap();
        assert_ne!(changed.identity, v4.identity);
    });
}

#[test]
fn portable_validator_rejects_every_required_malformed_class() {
    with_plan(|plan, scratch| {
        let identity_error = ExecutionPlan {
            identity: hash(99),
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&identity_error, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::IdentityMismatch
        );

        let duplicate_nodes = [plan.nodes[0], plan.nodes[0]];
        let duplicate = ExecutionPlan {
            nodes: &duplicate_nodes,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&duplicate, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::DuplicateIdentity
        );

        let dangling_cords = [ResolvedPlanCord {
            from: ResolvedPlanPort {
                node: InstancePath::new("root/missing").unwrap(),
                ..plan.cords[0].from
            },
            ..plan.cords[0]
        }];
        let dangling = ExecutionPlan {
            cords: &dangling_cords,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&dangling, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::DanglingReference
        );

        let mismatched_cords = [ResolvedPlanCord {
            to: ResolvedPlanPort {
                value_type: TypeContractRef {
                    semantic_hash: hash(88),
                    ..TYPE
                },
                ..plan.cords[0].to
            },
            ..plan.cords[0]
        }];
        let mismatched = ExecutionPlan {
            cords: &mismatched_cords,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&mismatched, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::ContractMismatch
        );

        let missing_nodes = [ResolvedPlanNode {
            artifact: Id("fixture/missing"),
            ..plan.nodes[0]
        }];
        let missing = ExecutionPlan {
            nodes: &missing_nodes,
            cords: &[],
            composites: &[],
            port_groups: &[],
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&missing, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::MissingArtifact
        );

        let over_budget = ExecutionPlan {
            budget: PlanResourceBudget::ZERO,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&over_budget, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::BudgetExceeded
        );

        let absent_grant = ExecutionPlan {
            authorities: &[],
            instance_pools: &[],
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&absent_grant, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::AuthorityInvalid
        );

        let absent_resource = ExecutionPlan {
            resources: &[],
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&absent_resource, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::DanglingReference
        );

        assert_eq!(
            validate_execution_plan(&plan, context(80), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::AuthorityInvalid
        );
        assert_eq!(
            validate_execution_plan(&plan, context(100), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::StaleHostObservation
        );

        let unsupported = ExecutionPlan {
            schema_version: 1,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&unsupported, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::UnsupportedVersion
        );

        let unresolved_items = [UnresolvedPlanConstraint {
            id: Id("fixture/selector"),
            requester: plan.nodes[0].instance,
            kind: UnresolvedPlanKind::Implementation,
        }];
        let unresolved = ExecutionPlan {
            supervisions: &[],
            unresolved: &unresolved_items,
            ..plan
        };
        let denial = validate_execution_plan(&unresolved, context(20), scratch).unwrap_err();
        assert_eq!(denial.code, PlanDiagnosticCode::UnresolvedSelection);
        assert_eq!(denial.collection, PlanCollection::Unresolved);

        let bad_queue_cords = [ResolvedPlanCord {
            queue_memory_bytes: 0,
            ..plan.cords[0]
        }];
        let bad_queue = ExecutionPlan {
            cords: &bad_queue_cords,
            ..plan
        };
        assert_eq!(
            validate_execution_plan(&bad_queue, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::QueueInvalid
        );

        assert_eq!(
            validate_execution_plan(&plan, context(20), &mut [])
                .unwrap_err()
                .code,
            PlanDiagnosticCode::ScratchTooSmall
        );
    });
}

#[test]
fn exact_evidence_provider_is_identity_bound_and_must_reference_exact_plan_facts() {
    with_plan(|plan, scratch| {
        let provider = PlanEvidenceProviderBinding {
            implementation: pin("fixture/evidence-provider", 91),
            artifact: plan.artifacts[0].id,
            host_observation: plan.host_observations[0].id,
            store: ResourceRef {
                kind: Id("fixture/evidence-store"),
                id: Id("fixture/evidence-store-a"),
            },
            store_generation: 7,
            grant_hash: hash(92),
            time_basis: plan.created_at.basis,
        };
        let mut bound = ExecutionPlan {
            identity: ZERO_HASH,
            evidence_provider: Some(provider),
            ..plan
        };
        bound.identity = bound.semantic_hash(scratch).unwrap();
        validate_execution_plan(&bound, context(20), scratch).unwrap();

        let mut changed = ExecutionPlan {
            identity: ZERO_HASH,
            evidence_provider: Some(PlanEvidenceProviderBinding {
                store_generation: 8,
                ..provider
            }),
            ..bound
        };
        changed.identity = changed.semantic_hash(scratch).unwrap();
        assert_ne!(changed.identity, bound.identity);

        let mut dangling = ExecutionPlan {
            identity: ZERO_HASH,
            evidence_provider: Some(PlanEvidenceProviderBinding {
                artifact: Id("fixture/missing-evidence-artifact"),
                ..provider
            }),
            ..bound
        };
        dangling.identity = dangling.semantic_hash(scratch).unwrap();
        let denial = validate_execution_plan(&dangling, context(20), scratch).unwrap_err();
        assert_eq!(denial.code, PlanDiagnosticCode::RuntimeEvidenceInvalid);
        assert_eq!(denial.collection, PlanCollection::EvidenceProvider);

        let mut invalid_generation = ExecutionPlan {
            identity: ZERO_HASH,
            evidence_provider: Some(PlanEvidenceProviderBinding {
                store_generation: 0,
                ..provider
            }),
            ..bound
        };
        invalid_generation.identity = invalid_generation.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&invalid_generation, context(20), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::RuntimeEvidenceInvalid
        );
    });
}

#[test]
fn current_plan_pins_value_clock_and_feedback_facts() {
    with_plan(|base, scratch| {
        let mut profile = ExecutionProfile {
            id: Id("fixture/execution-profile"),
            schema_version: 0,
            semantic_hash: ZERO_HASH,
            boundedness: BoundednessProfile::Hard,
            cancellation: CancellationGuarantee::Bounded,
            step_bound_enforced: true,
            limits: ExecutionLimits {
                max_step_work: 8,
                max_retained_values: 0,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_leases: 0,
                max_input_bytes: 0,
                max_output_reservations: 0,
                max_output_bytes: 0,
                max_transactions: 1,
                max_fragments_per_step: 0,
                max_pending_operations: 0,
                max_timers: 0,
                max_child_tasks: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_items: 0,
                max_foreign_queue_bytes: 0,
                max_checkpoint_bytes: 0,
                implementation_memory_bytes: 0,
                cancellation_ticks: 1,
            },
            representations: &[],
            memory_claims: &[],
            checkpoint: None,
        };
        profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
        let nodes = [
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..base.nodes[0]
            },
            ResolvedPlanNode {
                execution_profile: Some(&profile),
                ..base.nodes[1]
            },
        ];
        let clocks = [Id("fixture/clock")];
        let envelopes = [ValueEnvelopePolicy {
            cord: base.cords[0].id,
            representation: pin("fixture/bytes", 90),
            maximum_payload_bytes: base.cords[0].flow.capacity.max_value_bytes(),
            maximum_envelope_bytes: 16,
            maximum_fragments: 2,
            maximum_fragment_bytes: 16,
            maximum_timestamps: 1,
            clock_domains: &clocks,
            identity_allowed: true,
            correlation_allowed: true,
            causation_allowed: true,
            provenance_allowed: true,
            sensitivity_ceiling: Sensitivity::Restricted,
        }];
        let conversions = [PlanClockConversion {
            id: Id("fixture/clock-conversion"),
            source: Id("fixture/device-clock"),
            destination: clocks[0],
            numerator: 1,
            denominator: 1,
            offset_ticks: 0,
            rounding: ClockRounding::Exact,
            maximum_uncertainty_ticks: 1,
            observed_at: base.created_at,
            valid_until_tick: 50,
            authority: Id("fixture/clock-authority"),
        }];
        let boundaries = [PlanFeedbackBoundary {
            id: Id("fixture/feedback"),
            node: base.cords[0].to.node,
            cord: base.cords[0].id,
            kind: FeedbackBoundaryKind::Delay,
            initialization: FeedbackInitialization::Empty,
            initial_items: 0,
            initial_bytes: 0,
            maximum_retained_items: 1,
            maximum_retained_bytes: 32,
            delay_ticks: 1,
            clock: Some(clocks[0]),
            replay_gap: FeedbackReplayGapPolicy::Fail,
            cancellation: pin("fixture/cancellation", 91),
            terminal: FeedbackTerminalPolicy::DropRetained,
        }];
        let cords = [ResolvedPlanCord {
            queue_memory_bytes: base.cords[0].queue_memory_bytes
                + u64::from(envelopes[0].maximum_envelope_bytes)
                    * u64::from(base.cords[0].flow.capacity.items()),
            ..base.cords[0]
        }];
        let mut plan = ExecutionPlan {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            cords: &cords,
            value_envelopes: &envelopes,
            clock_conversions: &conversions,
            feedback_boundaries: &boundaries,
            nodes: &nodes,
            instance_pools: &[],
            ..base
        };
        plan.identity = plan.semantic_hash(scratch).unwrap();
        validate_execution_plan(
            &plan,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
                now: time(20),
            },
            scratch,
        )
        .unwrap();

        let watches = [WatchAdmission {
            id: Id("watch/cord-0"),
            subject: WatchSubject::Cord(cords[0].id),
            operator: Id("operator/fixture"),
            control_grant_hash: hash(90),
            lease: Id("lease/watch-cord-0"),
            representation: envelopes[0].representation,
            maximum_preview_bytes: 16,
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: WatchRetention::Latest,
            sensitivity_ceiling: Sensitivity::Public,
            reveal_action: None,
            reveal_grant_hash: None,
        }];
        let mut watched = ExecutionPlan {
            identity: ZERO_HASH,
            watch_admissions: &watches,
            ..plan
        };
        watched.identity = watched.semantic_hash(scratch).unwrap();
        validate_execution_plan(
            &watched,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
                now: time(20),
            },
            scratch,
        )
        .unwrap();
        assert_ne!(watched.identity, plan.identity);

        let without_feedback = ExecutionPlan {
            feedback_boundaries: &[],
            ..plan
        };
        assert_ne!(
            plan.identity,
            without_feedback.semantic_hash(scratch).unwrap()
        );

        let unauthorized_envelopes = [ValueEnvelopePolicy {
            maximum_timestamps: 0,
            clock_domains: &[],
            ..envelopes[0]
        }];
        let mut unauthorized_clock = ExecutionPlan {
            identity: ZERO_HASH,
            value_envelopes: &unauthorized_envelopes,
            ..plan
        };
        unauthorized_clock.identity = unauthorized_clock.semantic_hash(scratch).unwrap();
        let error = validate_execution_plan(
            &unauthorized_clock,
            PlanValidationContext {
                supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
                now: time(20),
            },
            scratch,
        )
        .unwrap_err();
        assert_eq!(error.code.as_str(), "CND-VEF-003");
        assert_eq!(error.collection, PlanCollection::FeedbackBoundaries);
    });
}

#[test]
fn current_plan_admits_a_watch_for_the_exact_cord_type_without_an_envelope() {
    with_plan(|base, scratch| {
        let value_type = base.cords[0].from.value_type;
        let watches = [WatchAdmission {
            id: Id("watch/plain-cord"),
            subject: WatchSubject::Cord(base.cords[0].id),
            operator: Id("operator/fixture"),
            control_grant_hash: hash(90),
            lease: Id("lease/watch-plain-cord"),
            representation: PinnedDescriptor {
                id: value_type.contract_id,
                schema_version: value_type.schema_version,
                semantic_hash: value_type.semantic_hash,
            },
            maximum_preview_bytes: base.cords[0].flow.capacity.max_value_bytes(),
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: WatchRetention::Latest,
            sensitivity_ceiling: Sensitivity::Public,
            reveal_action: None,
            reveal_grant_hash: None,
        }];
        let mut watched = ExecutionPlan {
            identity: ZERO_HASH,
            watch_admissions: &watches,
            ..base
        };
        watched.identity = watched.semantic_hash(scratch).unwrap();
        validate_execution_plan(&watched, context(10), scratch).unwrap();
    });
}
