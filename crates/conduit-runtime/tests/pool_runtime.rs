use conduit_core::{
    BoundednessProfile, CancellationGuarantee, EXECUTION_PLAN_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION_V15, ExecutionLimits, ExecutionProfile, Id,
    ImplementationMachine, InstancePath, InstancePhase, InstantiationContext, LifecycleUsage,
    PinnedDescriptor, PlanInstancePool, PlanPoolRuntime, PlanResourceBudget,
    PoolAdmissionDisposition, PoolAdmissionFacts, PoolAdmissionPolicy, PoolCleanupPolicy,
    PoolContract, PoolGenerationReservation, PoolReason, PoolReservationProfile, PoolSlotState,
    PoolSupervisionPolicy, PoolWorkIdentity, PrepareOutcome, SemanticHash, StepOutcome, StepUsage,
    prepare_all, start_all,
};
use conduit_runtime::{
    HostedPoolError, HostedPoolStepError, instantiate_plan_pool, observe_pool_step,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(name: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(name),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn profile(multiplier: u16) -> PoolReservationProfile {
    PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 128 * u64::from(multiplier),
            storage_bytes: 16 * u64::from(multiplier),
            cpu_units: u32::from(multiplier),
            timers: multiplier,
            transports: multiplier,
            checkpoints: multiplier,
            evidence_bytes: 64 * u64::from(multiplier),
        },
        child_nodes: 2 * multiplier,
        child_cords: multiplier,
        state_bytes: 32 * u64::from(multiplier),
        scheduler_slots: 3 * multiplier,
        host_operations: multiplier,
        cancellation_scopes: 2 * multiplier,
    }
}

fn plan_pool() -> PlanInstancePool<'static> {
    let pool = InstancePath::new("root/pool.workers").unwrap();
    let per_instance = profile(1);
    let total = profile(4);
    PlanInstancePool {
        instance: pool,
        template_hash: hash(1),
        derived_identity_hash: hash(2),
        maximum_live: 2,
        maximum_queued: 0,
        admission_policy: pin("fixture/admission", 3),
        supervision_policy: pin("fixture/supervision", 4),
        per_instance_budget: per_instance.resources,
        authority_grants: &[],
        maximum_instance_ticks: 100,
        implementation_set_hash: hash(5),
        correlation_slots: 2,
        worst_case_budget: total.resources,
        child_nodes: per_instance.child_nodes,
        child_cords: per_instance.child_cords,
        runtime: Some(PlanPoolRuntime {
            contract: PoolContract {
                pool,
                template_hash: hash(1),
                implementation_set_hash: hash(5),
                maximum_live: 2,
                maximum_queued: 0,
                admission: PoolAdmissionPolicy::Reject,
                supervision: PoolSupervisionPolicy::Isolate,
                cleanup: PoolCleanupPolicy::Abort,
                deadline_ticks: 100,
                idle_timeout_ticks: 20,
                cleanup_ticks: 5,
                reservation: per_instance,
                total_reservation: total,
                maximum_evidence_events: 64,
            },
            queued_reservation: PoolReservationProfile::default(),
            generation_reservation: PoolGenerationReservation {
                old_maximum_live: 2,
                candidate_maximum_live: 1,
                rollback_maximum_live: 1,
                reserved_slots: 4,
                per_instance,
                reserved_resources: total,
            },
        }),
    }
}

fn implementation_profile() -> ExecutionProfile<'static> {
    let mut profile = ExecutionProfile {
        id: Id("fixture/pool-child-profile"),
        schema_version: 1,
        semantic_hash: hash(0),
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: ExecutionLimits {
            max_step_work: 4,
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
    profile
}

fn started_machine<'a>(profile: &'a ExecutionProfile<'a>) -> ImplementationMachine<'a> {
    let mut machines = [ImplementationMachine::instantiate(
        profile,
        InstantiationContext {
            instance: InstancePath::new("root/pool.workers/child").unwrap(),
            implementation: pin("fixture/pool-child", 20),
            artifact: Id("artifact/pool-child"),
            execution_profile_hash: profile.semantic_hash,
            configuration_validated: true,
            caller_memory_bytes: 0,
            required_resource_bindings: &[],
            provided_resource_bindings: &[],
            required_grants: &[],
            provided_grants: &[],
            cancellation_scope: Id("scope/pool-child"),
        },
    )
    .unwrap()];
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
fn hosted_runtime_executes_the_exact_core_contract() {
    let pool = plan_pool();
    let mut runtime =
        instantiate_plan_pool::<2, 64>(EXECUTION_PLAN_SCHEMA_VERSION, hash(9), pool, 7, 1).unwrap();
    let facts = PoolAdmissionFacts {
        authority_granted: true,
        sensitivity_allowed: true,
        template_hash: hash(1),
        implementation_set_hash: hash(5),
        available: profile(1),
    };
    let work = PoolWorkIdentity {
        request: hash(10),
        work_unit: hash(11),
        correlation: hash(12),
    };
    assert_eq!(
        runtime.offer(work, facts, 0).unwrap(),
        PoolAdmissionDisposition::Started { slot: 0 }
    );
    runtime.mark_running(0, 0).unwrap();
    runtime.progress(0, 1).unwrap();
    runtime.complete(0, 2).unwrap();
    runtime.tick(7).unwrap();
    assert_eq!(runtime.population().terminal, 1);
}

#[test]
fn hosted_profile_and_legacy_plan_fail_before_admission() {
    let pool = plan_pool();
    assert!(matches!(
        instantiate_plan_pool::<1, 64>(EXECUTION_PLAN_SCHEMA_VERSION, hash(9), pool, 7, 1),
        Err(HostedPoolError::Contract(_))
    ));
    assert!(matches!(
        instantiate_plan_pool::<2, 64>(
            EXECUTION_PLAN_SCHEMA_VERSION,
            hash(9),
            PlanInstancePool {
                runtime: None,
                ..pool
            },
            7,
            1,
        ),
        Err(HostedPoolError::LegacyPlan)
    ));
    assert!(matches!(
        instantiate_plan_pool::<2, 64>(EXECUTION_PLAN_SCHEMA_VERSION_V15, hash(9), pool, 7, 1,),
        Err(HostedPoolError::LegacyPlan)
    ));
}

#[test]
fn host_neutral_child_steps_commit_atomically_to_pool_lifecycle() {
    let pool = plan_pool();
    let mut runtime =
        instantiate_plan_pool::<2, 64>(EXECUTION_PLAN_SCHEMA_VERSION, hash(9), pool, 7, 1).unwrap();
    let PoolAdmissionDisposition::Started { slot } = runtime
        .offer(
            PoolWorkIdentity {
                request: hash(30),
                work_unit: hash(31),
                correlation: hash(32),
            },
            PoolAdmissionFacts {
                authority_granted: true,
                sensitivity_allowed: true,
                template_hash: hash(1),
                implementation_set_hash: hash(5),
                available: profile(1),
            },
            0,
        )
        .unwrap()
    else {
        panic!("instance starts");
    };
    runtime.mark_running(slot, 0).unwrap();

    let profile = implementation_profile();
    let mut machine = started_machine(&profile);
    let observed = observe_pool_step(
        &mut runtime,
        slot,
        &mut machine,
        StepOutcome::Progress,
        StepUsage {
            work_units: 1,
            observable_operations: 1,
            ..StepUsage::default()
        },
        hash(90),
        1,
    )
    .unwrap();
    assert_eq!(
        observed.implementation.outcome(),
        conduit_core::StepOutcomeKind::Progress
    );
    assert_eq!(runtime.slots()[usize::from(slot)].last_progress_tick, 1);

    let before = machine.phase();
    let error = observe_pool_step(
        &mut runtime,
        slot,
        &mut machine,
        StepOutcome::Progress,
        StepUsage {
            work_units: 1,
            observable_operations: 1,
            retained_values: 1,
            ..StepUsage::default()
        },
        hash(91),
        2,
    )
    .unwrap_err();
    assert!(matches!(error, HostedPoolStepError::Implementation(_)));
    assert_eq!(machine.phase(), before);
    assert_eq!(machine.phase(), InstancePhase::Started);
    assert_eq!(
        runtime.slots()[usize::from(slot)].state,
        PoolSlotState::Cleanup
    );
    assert!(runtime.evidence().iter().any(|event| {
        event.reason == PoolReason::ForeignProfileExceeded && event.cause == Some(hash(91))
    }));
}
