use conduit_core::{
    ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime, BlockingFairness,
    DelegationPolicy, Direction, EffectRequirement, ExecutionPlan, FlowCapacity, FlowPolicy,
    FlowWatermarks, GrantStatus, HostCapability, Id, InstancePath, ObservedGrant, PinnedDescriptor,
    PlanArtifact, PlanAuthority, PlanCollection, PlanCompositeMapping, PlanDiagnosticCode,
    PlanExportBinding, PlanHostObservation, PlanInstancePool, PlanPortGroup, PlanPortGroupMember,
    PlanResourceBinding, PlanResourceBudget, PlanValidationContext, Pressure, ResolvedPlanCord,
    ResolvedPlanNode, ResolvedPlanPort, ResourceRef, ResourceSelector, SemanticHash, StopPolicy,
    TypeContractRef, UnresolvedPlanConstraint, UnresolvedPlanKind, resolve_authority,
    validate_execution_plan,
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

fn with_plan(test: impl FnOnce(ExecutionPlan<'_>, &mut [SemanticHash; 32])) {
    let effect = EffectRequirement {
        id: Id("read"),
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
        authorities: &authorities,
        composites: &composites,
        port_groups: &groups,
        instance_pools: &pools,
        unresolved: &[],
    };
    let mut scratch = [ZERO_HASH; 32];
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
