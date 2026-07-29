use conduit_core::{
    ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime, BlockingFairness,
    BoundednessProfile, CancellationCheckpointPolicy, CancellationGuarantee,
    CheckpointProviderCapabilities, DelegationPolicy, DeliveryClaim, DescriptorRef, Direction,
    DuplicatePolicy, DuplicationRule, EffectRequirement, EventClass, EventProviderCapabilities,
    EventStreamContract, ExecutionLimits, ExecutionPlan, ExecutionProfile,
    ExplicitSatisfactionRequirement, FanOutMode, FlowCapacity, FlowPolicy, FlowWatermarks,
    GrantStatus, HostCapability, Id, InstancePath, JobContract, MergeOrdering, MergeTerminalPolicy,
    ObservedGrant, PinnedDescriptor, PlanArtifact, PlanAuthority, PlanCollection,
    PlanCompositeMapping, PlanDiagnosticCode, PlanEventStream, PlanExportBinding, PlanFanOut,
    PlanHostObservation, PlanInstancePool, PlanJob, PlanMerge, PlanMergeInput, PlanPortGroup,
    PlanPortGroupMember, PlanResourceBinding, PlanResourceBudget, PlanSatisfactionProof,
    PlanSatisfactionSubject, PlanValidationContext, Pressure, RUNTIME_EVIDENCE_POLICY_VERSION,
    ReplayDelivery, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, ResourceRef,
    ResourceSelector, RestartPolicy, RetentionPolicy, RuntimeEvidenceMode, RuntimeEvidencePolicy,
    SatisfactionFacet, SatisfactionMethod, SatisfactionObligation, SatisfactionPin,
    SatisfactionProof, SatisfactionReason, SatisfactionRole, SemanticHash, Sensitivity, StopPolicy,
    SubscriberCoupling, TypeContractRef, UnresolvedPlanConstraint, UnresolvedPlanKind,
    resolve_authority, validate_execution_plan,
};

const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
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
        schema_version: 1,
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
    };
    let required_effects = [effect_hash];
    let required_resources = [Id("fixture/source-device")];
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
    }];
    let nodes = [
        ResolvedPlanNode {
            instance: InstancePath::new("root/source").unwrap(),
            contract: pin("fixture/source-contract", 13),
            implementation: pin("fixture/source-impl", 14),
            lifecycle_policy: pin("fixture/source-lifecycle", 28),
            execution_profile: None,
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
            execution_profile: None,
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
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(17),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: nodes[1].instance,
            port: Id("in"),
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
            boundary_port: Id("out"),
            member: nodes[0].instance,
            member_port: Id("out"),
            direction: Direction::Output,
        },
        PlanExportBinding {
            boundary_port: Id("in"),
            member: nodes[1].instance,
            member_port: Id("in"),
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
    let pool_grants = [grant.id];
    let pools = [PlanInstancePool {
        instance: InstancePath::new("root/pool").unwrap(),
        template_hash: hash(23),
        derived_identity_hash: hash(24),
        maximum_live: 1,
        maximum_queued: 1,
        admission_policy: pin("fixture/admission", 25),
        supervision_policy: pin("fixture/supervision", 26),
        per_instance_budget: PlanResourceBudget {
            memory_bytes: 16,
            timers: 1,
            evidence_bytes: 16,
            ..PlanResourceBudget::ZERO
        },
        authority_grants: &pool_grants,
        maximum_instance_ticks: 50,
        implementation_set_hash: hash(27),
        correlation_slots: 2,
        worst_case_budget: PlanResourceBudget {
            memory_bytes: 16,
            timers: 1,
            evidence_bytes: 16,
            ..PlanResourceBudget::ZERO
        },
        child_nodes: 2,
        child_cords: 1,
    }];
    let authorities = [authority];
    let mut plan = ExecutionPlan {
        schema_version: 1,
        identity: ZERO_HASH,
        source_semantic_hash: hash(1),
        resolver: pin("fixture/resolver", 2),
        resolver_policy_hash: hash(3),
        created_at: time(10),
        budget: PLAN_BUDGET,
        host_observations: &observations,
        resources: &resources,
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &authorities,
        hazard_closure: None,
        composites: &composites,
        port_groups: &groups,
        instance_pools: &pools,
        unresolved: &[],
    };
    let mut scratch = [ZERO_HASH; 64];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    test(plan, &mut scratch);
}

fn context(tick: u64) -> PlanValidationContext<'static> {
    PlanValidationContext {
        supported_schema_version: 1,
        now: time(tick),
    }
}

#[test]
fn valid_nested_plan_pins_every_runnable_boundary() {
    with_plan(|plan, scratch| {
        assert_eq!(
            plan.identity.to_string(),
            "sha256:5e0b490b723828de2fa235ec8cdc5338bc9c1263cc191a770a5b49c66a2e28a7"
        );
        assert_eq!(validate_execution_plan(&plan, context(20), scratch), Ok(()));
        assert_eq!(plan.identity, plan.semantic_hash(scratch).unwrap());
        assert_eq!(plan.instance_pools[0].correlation_slots, 2);
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

        let fixture = include_str!("../../../conformance/c2/execution-plan-v1.tsv");
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
fn plan_v2_pins_group_maximum_and_direction_without_rewriting_v1() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c2/port-group-correlation-v1.json");
        for case in [
            "plan-v1-preserved",
            "plan-v2-maximum",
            "plan-v2-direction",
            "plan-v2-membership-over-maximum",
            "plan-v1-to-v2-migration",
        ] {
            assert!(fixture.contains(&format!("\"id\": \"{case}\"")));
        }
        let v1_changed_group = [PlanPortGroup {
            maximum: 99,
            direction: Direction::Input,
            ..plan.port_groups[0]
        }];
        let v1_changed = ExecutionPlan {
            port_groups: &v1_changed_group,
            ..plan
        };
        assert_eq!(v1_changed.semantic_hash(scratch).unwrap(), plan.identity);

        let mut v2 = ExecutionPlan {
            schema_version: 2,
            identity: ZERO_HASH,
            ..plan
        };
        v2.identity = v2.semantic_hash(scratch).unwrap();
        let v2_context = PlanValidationContext {
            supported_schema_version: 2,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v2, v2_context, scratch), Ok(()));
        assert_ne!(v2.identity, plan.identity);

        let changed_maximum_group = [PlanPortGroup {
            maximum: 3,
            ..v2.port_groups[0]
        }];
        let changed_maximum = ExecutionPlan {
            port_groups: &changed_maximum_group,
            ..v2
        };
        assert_ne!(changed_maximum.semantic_hash(scratch).unwrap(), v2.identity);

        let changed_direction_group = [PlanPortGroup {
            direction: Direction::Input,
            ..v2.port_groups[0]
        }];
        let changed_direction = ExecutionPlan {
            port_groups: &changed_direction_group,
            ..v2
        };
        assert_ne!(
            changed_direction.semantic_hash(scratch).unwrap(),
            v2.identity
        );

        let invalid_group = [PlanPortGroup {
            maximum: 1,
            ..v2.port_groups[0]
        }];
        let mut invalid = ExecutionPlan {
            identity: ZERO_HASH,
            port_groups: &invalid_group,
            ..v2
        };
        invalid.identity = invalid.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&invalid, v2_context, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::InvalidDescriptor
        );
        assert_eq!(
            validate_execution_plan(&v2, context(1), scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::UnsupportedVersion
        );
    });
}

#[test]
fn plan_v3_pins_bounded_execution_profiles_without_rewriting_v1_or_v2() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c4/implementation-step-v1.json");
        for case in [
            "plan-v3-profile-pinned",
            "plan-v1-v2-identities-preserved",
            "plan-v3-missing-profile-rejected",
            "plan-v2-profile-rejected",
        ] {
            assert!(fixture.contains(&format!("\"id\":\"{case}\"")));
        }
        let mut profile = ExecutionProfile {
            id: Id("fixture/execution-profile"),
            schema_version: 1,
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
        let mut v3 = ExecutionPlan {
            schema_version: 3,
            identity: ZERO_HASH,
            nodes: &nodes,
            ..plan
        };
        v3.identity = v3.semantic_hash(scratch).unwrap();
        let v3_context = PlanValidationContext {
            supported_schema_version: 3,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v3, v3_context, scratch), Ok(()));
        assert_ne!(v3.identity, plan.identity);

        let mut v2_with_profile = ExecutionPlan {
            schema_version: 2,
            identity: ZERO_HASH,
            nodes: &nodes,
            ..plan
        };
        v2_with_profile.identity = v2_with_profile.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(
                &v2_with_profile,
                PlanValidationContext {
                    supported_schema_version: 2,
                    now: time(20)
                },
                scratch
            )
            .unwrap_err()
            .code,
            PlanDiagnosticCode::InvalidDescriptor
        );

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
            ..v3
        };
        missing.identity = missing.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&missing, v3_context, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::InvalidDescriptor
        );
    });
}

#[test]
fn plan_v4_pins_coupled_fanout_and_deterministic_merge_without_rewriting_v3() {
    with_plan(|plan, scratch| {
        let fixture = include_str!("../../../conformance/c4/structural-flow-v1.json");
        for case in [
            "plan-v4-coupled-fanout-pinned",
            "plan-v4-multi-edge-without-fanout-rejected",
            "non-copyable-value-rejected",
            "deterministic-round-robin",
            "event-time-late-value",
            "plan-v1-v3-identities-preserved",
        ] {
            assert!(fixture.contains(&format!("\"id\":\"{case}\"")));
        }
        let resonance = include_str!("../../../conformance/c4/resonance-v1.json");
        for case in [
            "plan-v5-stream-identity",
            "plan-v1-v4-identities-preserved",
            "durability-provider-rejected",
            "retention-provider-rejected",
            "security-provider-rejected",
        ] {
            assert!(resonance.contains(&format!("\"id\":\"{case}\"")));
        }
        let mut profile = ExecutionProfile {
            id: Id("fixture/structural-profile"),
            schema_version: 1,
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
            schema_version: 4,
            identity: ZERO_HASH,
            nodes: &nodes,
            cords: &cords,
            fanouts: &fanouts,
            merges: &merges,
            ..plan
        };
        v4.identity = v4.semantic_hash(scratch).unwrap();
        let context = PlanValidationContext {
            supported_schema_version: 4,
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
            schema_version: 5,
            identity: ZERO_HASH,
            event_streams: &[stream],
            runtime_evidence: None,
            ..v4
        };
        v5.identity = v5.semantic_hash(scratch).unwrap();
        let context5 = PlanValidationContext {
            supported_schema_version: 5,
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
            ..v5
        };
        incapable.identity = incapable.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&incapable, context5, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::EventStreamInvalid
        );

        let mut illegal_v4_stream = ExecutionPlan {
            schema_version: 4,
            identity: ZERO_HASH,
            ..v5
        };
        illegal_v4_stream.identity = illegal_v4_stream.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(
                &illegal_v4_stream,
                PlanValidationContext {
                    supported_schema_version: 4,
                    now: time(20)
                },
                scratch
            )
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
            schema_version: 6,
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
            jobs: &jobs,
            ..v5
        };
        v6.identity = v6.semantic_hash(scratch).unwrap();
        let context6 = PlanValidationContext {
            supported_schema_version: 6,
            now: time(20),
        };
        assert_eq!(validate_execution_plan(&v6, context6, scratch), Ok(()));
        assert_ne!(v6.identity, v5.identity);

        let alternate_type = TypeContractRef {
            contract_id: Id("fixture/alternate-value"),
            schema_version: 1,
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
            schema_version: 1,
            identity: ZERO_HASH,
            role: SatisfactionRole::PortConnection,
            method: SatisfactionMethod::StructuralFacets,
            required: DescriptorRef {
                kind: Id("conduit/port-contract"),
                schema_version: 1,
                semantic_hash: structural_cords[0].to.port_contract_hash,
            },
            offered: DescriptorRef {
                kind: Id("conduit/port-contract"),
                schema_version: 1,
                semantic_hash: structural_cords[0].from.port_contract_hash,
            },
            provider: Some(SatisfactionPin {
                descriptor: DescriptorRef {
                    kind: Id("fixture/type-provider"),
                    schema_version: 1,
                    semantic_hash: hash(94),
                },
            }),
            provider_rule: Some(Id("fixture/structural-port-v1")),
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
            schema_version: 1,
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
                    schema_version: 1,
                    semantic_hash: hash(96),
                },
            }),
            provider_rule: Some(Id("fixture/implementation-satisfies-v1")),
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
            schema_version: 7,
            identity: ZERO_HASH,
            cords: &structural_cords,
            fanouts: &structural_fanouts,
            satisfaction_proofs: &proof_bindings,
            ..v6
        };
        v7.identity = v7.semantic_hash(scratch).unwrap();
        let context7 = PlanValidationContext {
            supported_schema_version: 7,
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

        let mut illegal_v6_proof = ExecutionPlan {
            schema_version: 6,
            identity: ZERO_HASH,
            cords: v6.cords,
            fanouts: v6.fanouts,
            ..v7
        };
        illegal_v6_proof.identity = illegal_v6_proof.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&illegal_v6_proof, context6, scratch)
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
            schema_version: 8,
            identity: ZERO_HASH,
            runtime_evidence: Some(runtime_policy),
            ..v7
        };
        v8.identity = v8.semantic_hash(scratch).unwrap();
        let context8 = PlanValidationContext {
            supported_schema_version: 8,
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

        let mut illegal_v7_evidence = ExecutionPlan {
            schema_version: 7,
            identity: ZERO_HASH,
            ..v8
        };
        illegal_v7_evidence.identity = illegal_v7_evidence.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&illegal_v7_evidence, context7, scratch)
                .unwrap_err()
                .code,
            PlanDiagnosticCode::RuntimeEvidenceInvalid
        );

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

        let mut illegal_v5_job = ExecutionPlan {
            schema_version: 5,
            identity: ZERO_HASH,
            ..v6
        };
        illegal_v5_job.identity = illegal_v5_job.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(&illegal_v5_job, context5, scratch)
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

        let mut illegal_v3 = ExecutionPlan {
            schema_version: 3,
            identity: ZERO_HASH,
            ..v4
        };
        illegal_v3.identity = illegal_v3.semantic_hash(scratch).unwrap();
        assert_eq!(
            validate_execution_plan(
                &illegal_v3,
                PlanValidationContext {
                    supported_schema_version: 3,
                    now: time(20)
                },
                scratch
            )
            .unwrap_err()
            .code,
            PlanDiagnosticCode::StructuralInvalid
        );
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
            schema_version: 2,
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
