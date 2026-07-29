use conduit_core::{
    BoundednessProfile, CancellationGuarantee, Direction, ExecutionLimits, ExecutionProfile,
    HandleDisposition, HostOperationContext, HostOperationRequest, Id, ImplementationError,
    ImplementationMachine, InstancePath, InstancePhase, InstantiationContext, LifecycleUsage,
    MemoryAccounting, MemoryCategory, MemoryClaim, OwnershipModel, PinnedDescriptor,
    PlanResourceBudget, PortTransaction, PrepareOutcome, PublicationMode, SemanticHash,
    StepOutcome, StepOutcomeKind, StepUsage, TransactionState, TypeContractRef,
    ValueRepresentation, WakeInterest, WakeInterestKind, prepare_all, start_all,
    validate_host_operation, validate_plan_execution_profile,
};

const FIXTURE: &str = include_str!("../../../conformance/c4/implementation-step-v1.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([1; 32]),
};
const REPRESENTATION: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/representation"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([2; 32]),
};
const INPUT_A: ValueRepresentation<'static> = ValueRepresentation {
    direction: Direction::Input,
    port: Id("a"),
    semantic_type: TYPE,
    representation: REPRESENTATION,
    ownership: OwnershipModel::Borrowed,
    disposition: HandleDisposition::None,
    max_bytes: 8,
};
const INPUT_B: ValueRepresentation<'static> = ValueRepresentation {
    port: Id("b"),
    ..INPUT_A
};
const OUTPUT_A: ValueRepresentation<'static> = ValueRepresentation {
    direction: Direction::Output,
    port: Id("out-a"),
    semantic_type: TYPE,
    representation: REPRESENTATION,
    ownership: OwnershipModel::Owned,
    disposition: HandleDisposition::None,
    max_bytes: 16,
};
const OUTPUT_B: ValueRepresentation<'static> = ValueRepresentation {
    port: Id("out-b"),
    ..OUTPUT_A
};
const HANDLE: ValueRepresentation<'static> = ValueRepresentation {
    direction: Direction::Input,
    port: Id("device"),
    semantic_type: TYPE,
    representation: REPRESENTATION,
    ownership: OwnershipModel::ExclusiveHandle,
    disposition: HandleDisposition::ExplicitDispose,
    max_bytes: 8,
};
const REPRESENTATIONS: &[ValueRepresentation<'static>] =
    &[INPUT_A, INPUT_B, OUTPUT_A, OUTPUT_B, HANDLE];
const CLAIMS: &[MemoryClaim] = &[
    MemoryClaim {
        category: MemoryCategory::Retained,
        accounting: MemoryAccounting::ExecutorAllocated,
        bytes: 64,
    },
    MemoryClaim {
        category: MemoryCategory::StepScratch,
        accounting: MemoryAccounting::ExecutorAllocated,
        bytes: 32,
    },
    MemoryClaim {
        category: MemoryCategory::PortTransactions,
        accounting: MemoryAccounting::ExecutorAllocated,
        bytes: 48,
    },
    MemoryClaim {
        category: MemoryCategory::PendingOperations,
        accounting: MemoryAccounting::BackendBounded,
        bytes: 16,
    },
    MemoryClaim {
        category: MemoryCategory::HostServices,
        accounting: MemoryAccounting::ExternallyBounded,
        bytes: 32,
    },
    MemoryClaim {
        category: MemoryCategory::ForeignRuntime,
        accounting: MemoryAccounting::BackendBounded,
        bytes: 64,
    },
];
const LIMITS: ExecutionLimits = ExecutionLimits {
    max_step_work: 8,
    max_retained_values: 2,
    max_retained_bytes: 32,
    max_scratch_bytes: 16,
    max_input_leases: 2,
    max_input_bytes: 16,
    max_output_reservations: 2,
    max_output_bytes: 32,
    max_transactions: 2,
    max_fragments_per_step: 2,
    max_pending_operations: 1,
    max_timers: 1,
    max_child_tasks: 1,
    max_host_buffer_bytes: 16,
    max_foreign_queue_items: 2,
    max_foreign_queue_bytes: 8,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 256,
    cancellation_ticks: 10,
};

fn with_profile(test: impl FnOnce(ExecutionProfile<'static>)) {
    let mut profile = ExecutionProfile {
        id: Id("fixture/execution-profile"),
        schema_version: 1,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: LIMITS,
        representations: REPRESENTATIONS,
        memory_claims: CLAIMS,
        checkpoint: None,
    };
    let mut scratch = [ZERO; 16];
    profile.semantic_hash = profile.computed_semantic_hash(&mut scratch).unwrap();
    test(profile);
}

fn instantiation(profile: &ExecutionProfile<'_>) -> InstantiationContext<'static> {
    InstantiationContext {
        instance: InstancePath::new("root/node").unwrap(),
        implementation: PinnedDescriptor {
            id: Id("fixture/implementation"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([3; 32]),
        },
        artifact: Id("artifact/a"),
        execution_profile_hash: profile.semantic_hash,
        configuration_validated: true,
        caller_memory_bytes: 144,
        required_resource_bindings: &[],
        provided_resource_bindings: &[],
        required_grants: &[],
        provided_grants: &[],
        cancellation_scope: Id("scope/a"),
    }
}

fn started<'a>(profile: &'a ExecutionProfile<'a>) -> ImplementationMachine<'a> {
    let mut machines =
        [ImplementationMachine::instantiate(profile, instantiation(profile)).unwrap()];
    prepare_all(
        &mut machines,
        &[PrepareOutcome::Ready],
        &[LifecycleUsage::default()],
    )
    .unwrap();
    start_all(&mut machines, &[LifecycleUsage::default()]).unwrap();
    machines[0]
}

#[test]
fn instantiation_requires_exact_config_memory_resources_authority_and_profile() {
    with_profile(|profile| {
        assert!(ImplementationMachine::instantiate(&profile, instantiation(&profile)).is_ok());
        for invalid in [
            InstantiationContext {
                configuration_validated: false,
                ..instantiation(&profile)
            },
            InstantiationContext {
                caller_memory_bytes: 143,
                ..instantiation(&profile)
            },
            InstantiationContext {
                required_resource_bindings: &[Id("resource/a")],
                ..instantiation(&profile)
            },
            InstantiationContext {
                provided_grants: &[Id("grant/ambient")],
                ..instantiation(&profile)
            },
            InstantiationContext {
                execution_profile_hash: SemanticHash::from_bytes([99; 32]),
                ..instantiation(&profile)
            },
        ] {
            assert!(matches!(
                ImplementationMachine::instantiate(&profile, invalid),
                Err(ImplementationError::InstantiationViolation)
            ));
        }
    });
}

#[test]
fn profile_identity_bounds_and_representation_are_exact() {
    with_profile(|profile| {
        let mut scratch = [ZERO; 16];
        assert_eq!(
            validate_plan_execution_profile(
                &profile,
                PlanResourceBudget {
                    memory_bytes: 256,
                    timers: 1,
                    ..PlanResourceBudget::ZERO
                },
                &mut scratch
            ),
            Ok(())
        );
        assert_ne!(
            profile.representations[0].semantic_type.semantic_hash,
            profile.representations[0].representation.semantic_hash
        );
        assert_eq!(
            profile.representations[4].disposition,
            HandleDisposition::ExplicitDispose
        );
        let reversed_representations = REPRESENTATIONS.iter().rev().copied().collect::<Vec<_>>();
        let reversed_claims = CLAIMS.iter().rev().copied().collect::<Vec<_>>();
        let reordered = ExecutionProfile {
            representations: &reversed_representations,
            memory_claims: &reversed_claims,
            ..profile
        };
        assert_eq!(
            reordered.computed_semantic_hash(&mut scratch).unwrap(),
            profile.semantic_hash
        );

        let altered = ExecutionProfile {
            limits: ExecutionLimits {
                max_step_work: 7,
                ..profile.limits
            },
            ..profile
        };
        assert_eq!(
            altered.validate(&mut scratch),
            Err(ImplementationError::ProfileIdentityMismatch)
        );

        let observed_claims = [MemoryClaim {
            accounting: MemoryAccounting::ObservedOnly,
            ..CLAIMS[0]
        }];
        let observed = ExecutionProfile {
            memory_claims: &observed_claims,
            limits: ExecutionLimits {
                implementation_memory_bytes: 64,
                max_retained_bytes: 0,
                max_scratch_bytes: 0,
                max_input_bytes: 0,
                max_output_bytes: 0,
                max_host_buffer_bytes: 0,
                max_foreign_queue_bytes: 0,
                ..profile.limits
            },
            ..profile
        };
        assert_eq!(
            observed.validate(&mut scratch),
            Err(ImplementationError::InvalidProfile)
        );
        for invalid in [
            ExecutionProfile {
                step_bound_enforced: false,
                ..profile
            },
            ExecutionProfile {
                cancellation: CancellationGuarantee::Unbounded,
                ..profile
            },
        ] {
            assert_eq!(
                invalid.validate(&mut scratch),
                Err(ImplementationError::InvalidProfile)
            );
        }
    });
}

#[test]
fn prepare_all_is_atomic_and_start_is_separate() {
    with_profile(|profile| {
        let mut machines = [
            ImplementationMachine::instantiate(&profile, instantiation(&profile)).unwrap(),
            ImplementationMachine::instantiate(&profile, instantiation(&profile)).unwrap(),
        ];
        let failed = [
            PrepareOutcome::Ready,
            PrepareOutcome::Failed {
                code: Id("fixture/prepare-failed"),
            },
        ];
        assert_eq!(
            prepare_all(&mut machines, &failed, &[LifecycleUsage::default(); 2]),
            Err(ImplementationError::PrepareFailed)
        );
        assert!(
            machines
                .iter()
                .all(|machine| machine.phase() == InstancePhase::Instantiated)
        );
        prepare_all(
            &mut machines,
            &[PrepareOutcome::Ready, PrepareOutcome::Ready],
            &[LifecycleUsage::default(); 2],
        )
        .unwrap();
        assert!(
            machines
                .iter()
                .all(|machine| machine.phase() == InstancePhase::Prepared)
        );
        start_all(&mut machines, &[LifecycleUsage::default(); 2]).unwrap();
        assert!(
            machines
                .iter()
                .all(|machine| machine.phase() == InstancePhase::Started)
        );

        let mut excessive_prepare =
            [ImplementationMachine::instantiate(&profile, instantiation(&profile)).unwrap()];
        assert_eq!(
            prepare_all(
                &mut excessive_prepare,
                &[PrepareOutcome::Ready],
                &[LifecycleUsage {
                    work_units: 9,
                    ..LifecycleUsage::default()
                }]
            ),
            Err(ImplementationError::PrepareFailed)
        );
        assert_eq!(excessive_prepare[0].phase(), InstancePhase::Instantiated);

        let mut excessive_start =
            [ImplementationMachine::instantiate(&profile, instantiation(&profile)).unwrap()];
        prepare_all(
            &mut excessive_start,
            &[PrepareOutcome::Ready],
            &[LifecycleUsage::default()],
        )
        .unwrap();
        assert_eq!(
            start_all(
                &mut excessive_start,
                &[LifecycleUsage {
                    pending_operations: 1,
                    ..LifecycleUsage::default()
                }]
            ),
            Err(ImplementationError::IllegalLifecycle)
        );
        assert_eq!(excessive_start[0].phase(), InstancePhase::Prepared);
    });
}

#[test]
fn exact_step_outcomes_interests_cancellation_and_evidence_are_enforced() {
    with_profile(|profile| {
        let mut machine = started(&profile);
        let observation = machine
            .observe_step(
                StepOutcome::Progress,
                StepUsage {
                    work_units: 1,
                    observable_operations: 1,
                    domain_evidence: 0,
                    ..StepUsage::default()
                },
            )
            .unwrap();
        assert_eq!(observation.outcome(), StepOutcomeKind::Progress);
        assert_eq!(observation.observable_operations(), 1);
        assert_eq!(observation.domain_evidence(), 0);

        assert_eq!(
            machine.observe_step(StepOutcome::Progress, StepUsage::default()),
            Err(ImplementationError::FalseProgress)
        );
        assert_eq!(
            machine.observe_step(
                StepOutcome::Yielded,
                StepUsage {
                    work_units: 7,
                    ..StepUsage::default()
                }
            ),
            Err(ImplementationError::FalseProgress)
        );

        for interest in [
            WakeInterest {
                kind: WakeInterestKind::Input,
                subject: Id("in"),
            },
            WakeInterest {
                kind: WakeInterestKind::Output,
                subject: Id("out"),
            },
            WakeInterest {
                kind: WakeInterestKind::HostOperation,
                subject: Id("host-op"),
            },
            WakeInterest {
                kind: WakeInterestKind::Timer,
                subject: Id("timer"),
            },
        ] {
            let mut pending = started(&profile);
            assert_eq!(
                pending
                    .observe_step(StepOutcome::Pending(&[interest]), StepUsage::default())
                    .unwrap()
                    .outcome(),
                StepOutcomeKind::Pending
            );
            pending.cancel().unwrap();
            assert_eq!(pending.phase(), InstancePhase::Cancelling);
        }
    });
}

#[test]
fn every_step_resource_ceiling_fails_closed() {
    with_profile(|profile| {
        let excessive = [
            StepUsage {
                scratch_bytes: 17,
                ..StepUsage::default()
            },
            StepUsage {
                retained_values: 3,
                ..StepUsage::default()
            },
            StepUsage {
                timers: 2,
                ..StepUsage::default()
            },
            StepUsage {
                child_tasks: 2,
                ..StepUsage::default()
            },
            StepUsage {
                pending_operations: 2,
                ..StepUsage::default()
            },
        ];
        for usage in excessive {
            let mut machine = started(&profile);
            assert_eq!(
                machine.observe_step(StepOutcome::Yielded, usage),
                Err(ImplementationError::StepBoundExceeded)
            );
        }
    });
}

#[test]
fn leases_reservations_joins_fragments_and_rollback_preserve_ownership() {
    with_profile(|profile| {
        let mut join = PortTransaction::new(&profile);
        join.lease_input(INPUT_A, 8).unwrap();
        join.lease_input(INPUT_B, 8).unwrap();
        join.reserve_output(OUTPUT_A, 8).unwrap();
        join.write_fragment(4).unwrap();
        join.write_fragment(4).unwrap();
        let committed = join.commit(PublicationMode::Atomic).unwrap();
        assert_eq!(committed.consumed_inputs, 2);
        assert_eq!(committed.published_bytes, 8);

        let mut full = PortTransaction::new(&profile);
        full.lease_input(INPUT_A, 8).unwrap();
        full.reserve_output(OUTPUT_A, 16).unwrap();
        assert_eq!(
            full.reserve_output(OUTPUT_B, 17),
            Err(ImplementationError::TransactionViolation)
        );
        let rollback = full.rollback().unwrap();
        assert_eq!(rollback.consumed_inputs, 0);
        assert_eq!(rollback.published_outputs, 0);
        assert_eq!(full.state(), TransactionState::RolledBack);

        let mut atomic = PortTransaction::new(&profile);
        atomic.reserve_output(OUTPUT_A, 4).unwrap();
        atomic.reserve_output(OUTPUT_B, 4).unwrap();
        atomic.write_fragment(8).unwrap();
        assert_eq!(
            atomic
                .commit(PublicationMode::Atomic)
                .unwrap()
                .published_outputs,
            2
        );

        for output in [OUTPUT_A, OUTPUT_B] {
            let mut independent = PortTransaction::new(&profile);
            independent.reserve_output(output, 4).unwrap();
            independent.write_fragment(4).unwrap();
            assert_eq!(
                independent
                    .commit(PublicationMode::Independent)
                    .unwrap()
                    .published_outputs,
                1
            );
        }
    });
}

#[test]
fn host_operations_and_optional_checkpoint_require_exact_plan_bindings() {
    with_profile(|profile| {
        let context = HostOperationContext {
            required_resource_bindings: &[Id("resource/a")],
            grant_ids: &[Id("grant/a")],
            now: conduit_core::AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 10,
            },
        };
        let request = HostOperationRequest {
            operation: Id("fixture/read"),
            resource_binding: Id("resource/a"),
            grant: Id("grant/a"),
            deadline: conduit_core::AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 15,
            },
            cancellation_scope: Id("scope/a"),
            buffer_bytes: 8,
            correlation: Id("request/a"),
        };
        assert_eq!(validate_host_operation(request, context, &profile), Ok(()));
        assert_eq!(
            validate_host_operation(
                HostOperationRequest {
                    resource_binding: Id("resource/ambient"),
                    ..request
                },
                context,
                &profile
            ),
            Err(ImplementationError::HostOperationViolation)
        );
        assert_eq!(
            profile.validate_checkpoint(conduit_core::CheckpointRequest {
                contract: PinnedDescriptor {
                    id: Id("fixture/checkpoint"),
                    schema_version: 1,
                    semantic_hash: SemanticHash::from_bytes([9; 32]),
                },
                maximum_bytes: 1,
            }),
            Err(ImplementationError::UnsupportedCheckpoint)
        );
    });
}

#[test]
fn fixture_inventory_is_complete_and_delegates_only_hosted_bindings() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 45);
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        assert!(ids.insert(case["id"].as_str().unwrap()));
        assert!(case["expected"].is_object());
    }
    let delegated = cases
        .iter()
        .filter(|case| case["runner"] == "hosted-bindings")
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        delegated,
        [
            "native-message-binding-equivalence",
            "foreign-protocol-version-rejected"
        ]
    );
}
