use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use conduit_core::{
    ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime, BlockingFairness,
    BoundednessProfile, CancellationGuarantee, DelegationPolicy, Direction, DuplicationRule,
    EffectRequirement, EventClass, EventCorrelation, EventProviderCapabilities,
    EventStreamContract, EvidencePolicy, EvidenceStreamExtension, ExecutionLimits, ExecutionPlan,
    ExecutionProfile, FanOutMode, FlowCapacity, FlowPolicy, FlowQueueState, FlowWatermarks,
    GrantStatus, HostCapability, Id, ImplementationMachine, InstancePath, InstantiationContext,
    LifecycleUsage, MemoryAccounting, MemoryCategory, MemoryClaim, ObservedGrant, PinnedDescriptor,
    PlanArtifact, PlanAuthority, PlanCompositeMapping, PlanEventStream, PlanExportBinding,
    PlanFanOut, PlanHostObservation, PlanResourceBinding, PlanResourceBudget,
    PlanValidationContext, Pressure, RUNTIME_EVIDENCE_POLICY_VERSION, ReadyQueueDiscipline,
    ReplayDelivery, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, ResourceRef,
    ResourceSelector, RetentionPolicy, RuntimeEvidenceMode, RuntimeEvidencePolicy,
    SCHEDULER_CONTRACT_VERSION, SampleSchedule, SchedulerDecisionReason, SchedulerPolicy,
    SemanticHash, Sensitivity, StopPolicy, SubscriberCoupling, TypeContractRef,
    extend_execution_event, resolve_authority,
};
use conduit_runtime::{
    DeterministicExecutor, OwnedEventPayload, RuntimeEvidenceContext, RuntimeValue, ScheduledNode,
    SchedulerError, SchedulerEventKind, SchedulerNode, SchedulerReservation, SchedulerStatus,
    SchedulerStep, SendStatus, StepIo, record_scheduler_evidence,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([9; 32]),
};
const CLAIMS: [MemoryClaim; 2] = [
    MemoryClaim {
        category: MemoryCategory::PortTransactions,
        accounting: MemoryAccounting::ExecutorAllocated,
        bytes: 256,
    },
    MemoryClaim {
        category: MemoryCategory::PendingOperations,
        accounting: MemoryAccounting::ExecutorAllocated,
        bytes: 64,
    },
];
const LIMITS: ExecutionLimits = ExecutionLimits {
    max_step_work: 4,
    max_retained_values: 0,
    max_retained_bytes: 0,
    max_scratch_bytes: 0,
    max_input_leases: 2,
    max_input_bytes: 128,
    max_output_reservations: 2,
    max_output_bytes: 128,
    max_transactions: 1,
    max_fragments_per_step: 2,
    max_pending_operations: 1,
    max_timers: 1,
    max_child_tasks: 0,
    max_host_buffer_bytes: 0,
    max_foreign_queue_items: 0,
    max_foreign_queue_bytes: 0,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 320,
    cancellation_ticks: 8,
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

fn profile() -> ExecutionProfile<'static> {
    let mut value = ExecutionProfile {
        id: Id("fixture/scheduler-profile"),
        schema_version: 1,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: LIMITS,
        representations: &[],
        memory_claims: &CLAIMS,
        checkpoint: None,
    };
    value.semantic_hash = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn policy(decisions: u64, events: u32) -> SchedulerPolicy {
    SchedulerPolicy {
        schema_version: SCHEDULER_CONTRACT_VERSION,
        ready_queue: ReadyQueueDiscipline::RoundRobin,
        max_decisions: decisions,
        max_tick: decisions + 100,
        max_consecutive_yields: 8,
        max_events: events,
    }
}

fn reservation() -> SchedulerReservation {
    SchedulerReservation {
        available_runtime_memory_bytes: 32_000_000,
        executor_overhead_limit_bytes: 31_000_000,
    }
}

fn with_plan(
    queue_items: u16,
    queue_bytes: u64,
    test: impl FnOnce(ExecutionPlan<'_>, &ExecutionProfile<'_>),
) {
    let profile = profile();
    let observation = [PlanHostObservation {
        id: Id("fixture/host-report"),
        host: Id("host/a"),
        semantic_hash: hash(1),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 1_000_000,
    }];
    let artifacts = [
        PlanArtifact {
            id: Id("fixture/source-artifact"),
            digest: ArtifactDigest::from_bytes([2; 32]),
        },
        PlanArtifact {
            id: Id("fixture/sink-artifact"),
            digest: ArtifactDigest::from_bytes([3; 32]),
        },
    ];
    let nodes = [
        ResolvedPlanNode {
            instance: InstancePath::new("root/source").unwrap(),
            contract: pin("fixture/source-contract", 4),
            implementation: pin("fixture/source-implementation", 5),
            lifecycle_policy: pin("fixture/lifecycle", 6),
            execution_profile: Some(&profile),
            artifact: artifacts[0].id,
            host_observation: observation[0].id,
            host: observation[0].host,
            allocation: PlanResourceBudget {
                memory_bytes: 512,
                cpu_units: 1,
                timers: 1,
                ..PlanResourceBudget::ZERO
            },
            required_resources: &[],
            required_effects: &[],
        },
        ResolvedPlanNode {
            instance: InstancePath::new("root/sink").unwrap(),
            contract: pin("fixture/sink-contract", 7),
            implementation: pin("fixture/sink-implementation", 8),
            lifecycle_policy: pin("fixture/lifecycle", 6),
            execution_profile: Some(&profile),
            artifact: artifacts[1].id,
            host_observation: observation[0].id,
            host: observation[0].host,
            allocation: PlanResourceBudget {
                memory_bytes: 512,
                cpu_units: 1,
                timers: 1,
                ..PlanResourceBudget::ZERO
            },
            required_resources: &[],
            required_effects: &[],
        },
    ];
    let capacity = FlowCapacity::new(queue_items, 64, queue_bytes).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, queue_items, capacity).unwrap(),
    )
    .unwrap();
    let cords = [ResolvedPlanCord {
        id: Id("values"),
        from: ResolvedPlanPort {
            node: nodes[0].instance,
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(10),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: nodes[1].instance,
            port: Id("in"),
            direction: Direction::Input,
            port_contract_hash: hash(11),
            value_type: TYPE,
        },
        flow,
        queue_memory_bytes: queue_bytes,
    }];
    let mut plan = ExecutionPlan {
        schema_version: 3,
        identity: ZERO,
        source_semantic_hash: hash(12),
        resolver: pin("fixture/resolver", 13),
        resolver_policy_hash: hash(14),
        created_at: AuthorityTime {
            basis: Id("clock/monotonic"),
            tick: 1,
        },
        budget: PlanResourceBudget {
            memory_bytes: 32_000_000,
            storage_bytes: 0,
            cpu_units: 2,
            timers: 2,
            transports: 0,
            checkpoints: 0,
            evidence_bytes: 32_000_000,
        },
        host_observations: &observation,
        resources: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &[],
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        unresolved: &[],
    };
    let mut scratch = [ZERO; 16];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    test(plan, &profile);
}

fn machine<'a>(
    profile: &'a ExecutionProfile<'a>,
    node: &ResolvedPlanNode<'a>,
) -> ImplementationMachine<'a> {
    ImplementationMachine::instantiate(
        profile,
        InstantiationContext {
            instance: node.instance,
            implementation: node.implementation,
            artifact: node.artifact,
            execution_profile_hash: profile.semantic_hash,
            configuration_validated: true,
            caller_memory_bytes: 320,
            required_resource_bindings: &[],
            provided_resource_bindings: &[],
            required_grants: &[],
            provided_grants: &[],
            cancellation_scope: Id("scope/run"),
        },
    )
    .unwrap()
}

#[derive(Clone)]
enum FixtureNode {
    Source {
        next: u64,
        total: u64,
        cord: usize,
        fragments_per_value: u16,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
        fail_prepare: bool,
    },
    CoupledSource {
        next: u64,
        total: u64,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    Sink {
        seen: Rc<RefCell<Vec<u64>>>,
        cord: usize,
        yields_remaining: u32,
        rollback_once: bool,
        rolled_back: bool,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    HostProgress {
        remaining: u64,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    YieldForever {
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    HostWait {
        waiting: bool,
        done: bool,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    Fail {
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    Join {
        seen: Rc<RefCell<Vec<(u64, u64)>>>,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
}

impl FixtureNode {
    fn source(total: u64) -> Self {
        Self::Source {
            next: 0,
            total,
            cord: 0,
            fragments_per_value: 0,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
            fail_prepare: false,
        }
    }

    fn source_on(total: u64, cord: usize) -> Self {
        Self::Source {
            next: 0,
            total,
            cord,
            fragments_per_value: 0,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
            fail_prepare: false,
        }
    }

    fn fragmented_source(total: u64) -> Self {
        Self::Source {
            next: 0,
            total,
            cord: 0,
            fragments_per_value: 2,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
            fail_prepare: false,
        }
    }

    fn sink(seen: Rc<RefCell<Vec<u64>>>) -> Self {
        Self::sink_on(seen, 0)
    }

    fn sink_on(seen: Rc<RefCell<Vec<u64>>>, cord: usize) -> Self {
        Self::Sink {
            seen,
            cord,
            yields_remaining: 0,
            rollback_once: false,
            rolled_back: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        }
    }

    fn counts(&self) -> (Rc<Cell<u32>>, Rc<Cell<u32>>) {
        match self {
            Self::Source {
                prepare_count,
                start_count,
                ..
            }
            | Self::CoupledSource {
                prepare_count,
                start_count,
                ..
            }
            | Self::Sink {
                prepare_count,
                start_count,
                ..
            }
            | Self::HostProgress {
                prepare_count,
                start_count,
                ..
            }
            | Self::YieldForever {
                prepare_count,
                start_count,
            }
            | Self::HostWait {
                prepare_count,
                start_count,
                ..
            }
            | Self::Fail {
                prepare_count,
                start_count,
            }
            | Self::Join {
                prepare_count,
                start_count,
                ..
            } => (prepare_count.clone(), start_count.clone()),
        }
    }
}

impl SchedulerNode for FixtureNode {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        let (prepare, _) = self.counts();
        prepare.set(prepare.get() + 1);
        if matches!(
            self,
            Self::Source {
                fail_prepare: true,
                ..
            }
        ) {
            Err(Id("fixture/prepare-failed"))
        } else {
            Ok(LifecycleUsage::default())
        }
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        let (_, start) = self.counts();
        start.set(start.get() + 1);
        Ok(LifecycleUsage::default())
    }

    fn step(&mut self, io: &mut StepIo<'_, '_>) -> SchedulerStep {
        match self {
            Self::Source {
                next,
                total,
                cord,
                fragments_per_value,
                ..
            } => {
                if *next == *total {
                    return SchedulerStep::Completed;
                }
                for _ in 0..*fragments_per_value {
                    io.record_fragment().unwrap();
                }
                let value = RuntimeValue {
                    handle: *next,
                    accounted_bytes: 8,
                };
                match io.send(*cord, value, None).unwrap() {
                    SendStatus::Reserved => {
                        *next += 1;
                        SchedulerStep::Progress
                    }
                    SendStatus::Dropped => {
                        *next += 1;
                        SchedulerStep::Progress
                    }
                    SendStatus::WouldBlock => {
                        io.wait_for_output(*cord).unwrap();
                        SchedulerStep::Pending
                    }
                    SendStatus::Terminated => SchedulerStep::Completed,
                    SendStatus::Rejected | SendStatus::Disconnected | SendStatus::Failed => {
                        SchedulerStep::Failed {
                            code: Id("fixture/output-not-published"),
                        }
                    }
                }
            }
            Self::CoupledSource { next, total, .. } => {
                if *next == *total {
                    return SchedulerStep::Completed;
                }
                let value = RuntimeValue {
                    handle: *next,
                    accounted_bytes: 8,
                };
                match io.send_coupled(0, &[value, value], &[None, None]).unwrap() {
                    SendStatus::Reserved => {
                        *next += 1;
                        SchedulerStep::Progress
                    }
                    SendStatus::WouldBlock => {
                        io.wait_for_output(0).unwrap();
                        io.wait_for_output(1).unwrap();
                        SchedulerStep::Pending
                    }
                    SendStatus::Terminated => SchedulerStep::Completed,
                    SendStatus::Rejected
                    | SendStatus::Dropped
                    | SendStatus::Disconnected
                    | SendStatus::Failed => SchedulerStep::Failed {
                        code: Id("fixture/coupled-output-not-published"),
                    },
                }
            }
            Self::Sink {
                seen,
                cord,
                yields_remaining,
                rollback_once,
                rolled_back,
                ..
            } => {
                if *yields_remaining > 0 {
                    *yields_remaining -= 1;
                    io.consume_work(4).unwrap();
                    return SchedulerStep::Yielded;
                }
                if let Some(value) = io.receive(*cord).unwrap() {
                    if *rollback_once && !*rolled_back {
                        *rolled_back = true;
                        io.wait_for_timer(Id("timer/retry"), io.tick() + 1).unwrap();
                        return SchedulerStep::Pending;
                    }
                    seen.borrow_mut().push(value.handle);
                    return SchedulerStep::Progress;
                }
                if matches!(
                    io.input_state(*cord).unwrap(),
                    FlowQueueState::Completed
                        | FlowQueueState::Cancelled
                        | FlowQueueState::Failed
                        | FlowQueueState::Disconnected
                ) {
                    SchedulerStep::Completed
                } else {
                    io.wait_for_input(*cord).unwrap();
                    SchedulerStep::Pending
                }
            }
            Self::HostProgress { remaining, .. } => {
                if *remaining == 0 {
                    SchedulerStep::Completed
                } else {
                    *remaining -= 1;
                    io.record_host_progress().unwrap();
                    SchedulerStep::Progress
                }
            }
            Self::YieldForever { .. } => {
                io.consume_work(4).unwrap();
                SchedulerStep::Yielded
            }
            Self::HostWait { waiting, done, .. } => {
                if *done {
                    SchedulerStep::Completed
                } else if !*waiting {
                    *waiting = true;
                    io.wait_for_host_operation(Id("operation/device")).unwrap();
                    SchedulerStep::Pending
                } else {
                    *done = true;
                    io.record_host_progress().unwrap();
                    SchedulerStep::Progress
                }
            }
            Self::Fail { .. } => SchedulerStep::Failed {
                code: Id("fixture/node-failed"),
            },
            Self::Join { seen, .. } => {
                let left = io.receive(0).unwrap();
                let right = io.receive(1).unwrap();
                match (left, right) {
                    (Some(left), Some(right)) => {
                        seen.borrow_mut().push((left.handle, right.handle));
                        SchedulerStep::Progress
                    }
                    (left, right) => {
                        let left_terminal = matches!(
                            io.input_state(0).unwrap(),
                            FlowQueueState::Completed
                                | FlowQueueState::Cancelled
                                | FlowQueueState::Failed
                                | FlowQueueState::Disconnected
                        );
                        let right_terminal = matches!(
                            io.input_state(1).unwrap(),
                            FlowQueueState::Completed
                                | FlowQueueState::Cancelled
                                | FlowQueueState::Failed
                                | FlowQueueState::Disconnected
                        );
                        if left.is_none() && right.is_none() && left_terminal && right_terminal {
                            SchedulerStep::Completed
                        } else {
                            if left.is_none() && !left_terminal {
                                io.wait_for_input(0).unwrap();
                            }
                            if right.is_none() && !right_terminal {
                                io.wait_for_input(1).unwrap();
                            }
                            SchedulerStep::Pending
                        }
                    }
                }
            }
        }
    }
}

fn start_executor<'a>(
    plan: &'a ExecutionPlan<'a>,
    profile: &'a ExecutionProfile<'a>,
    source: FixtureNode,
    sink: FixtureNode,
    scheduler_policy: SchedulerPolicy,
) -> Result<DeterministicExecutor<'a, FixtureNode>, SchedulerError> {
    let nodes = vec![
        ScheduledNode {
            driver: source,
            machine: machine(profile, &plan.nodes[0]),
        },
        ScheduledNode {
            driver: sink,
            machine: machine(profile, &plan.nodes[1]),
        },
    ];
    DeterministicExecutor::start(
        plan,
        PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        },
        scheduler_policy,
        reservation(),
        nodes,
    )
}

#[test]
fn exact_preallocation_and_atomic_startup() {
    with_plan(2, 128, |plan, profile| {
        let source = FixtureNode::source(1);
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let (source_prepare, source_start) = source.counts();
        let nodes = vec![
            ScheduledNode {
                driver: source,
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: sink,
                machine: machine(profile, &plan.nodes[1]),
            },
        ];
        let error = DeterministicExecutor::start(
            &plan,
            PlanValidationContext {
                supported_schema_version: 3,
                now: AuthorityTime {
                    basis: Id("clock/monotonic"),
                    tick: 2,
                },
            },
            policy(100, 1_000),
            SchedulerReservation {
                available_runtime_memory_bytes: 1,
                executor_overhead_limit_bytes: 1,
            },
            nodes,
        )
        .err()
        .unwrap();
        assert_eq!(error, SchedulerError::AllocationUnavailable);
        assert_eq!(source_prepare.get(), 0);
        assert_eq!(source_start.get(), 0);

        let executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(1),
            FixtureNode::sink(Rc::new(RefCell::new(Vec::new()))),
            policy(100, 1_000),
        )
        .unwrap();
        let allocation = executor.allocation();
        assert_eq!(allocation.queue_payload_bytes, 128);
        assert_eq!(allocation.queue_slots, 2);
        assert_eq!(allocation.ready_slots, 2);
        assert!(allocation.executor_overhead_bytes > 0);
        assert!(allocation.scheduler_evidence_bytes > 0);
        assert_eq!(executor.policy(), policy(100, 1_000));
        assert!(allocation.planned_memory_bytes + allocation.executor_overhead_bytes <= 32_000_000);

        let mut no_evidence_plan = ExecutionPlan {
            identity: ZERO,
            budget: PlanResourceBudget {
                evidence_bytes: 0,
                ..plan.budget
            },
            ..plan
        };
        let mut scratch = [ZERO; 16];
        no_evidence_plan.identity = no_evidence_plan.semantic_hash(&mut scratch).unwrap();
        let result = start_executor(
            &no_evidence_plan,
            profile,
            FixtureNode::source(1),
            FixtureNode::sink(Rc::new(RefCell::new(Vec::new()))),
            policy(100, 1_000),
        );
        assert_eq!(result.err(), Some(SchedulerError::AllocationExceedsPlan));
    });
}

#[test]
fn prepare_failure_starts_nothing() {
    with_plan(1, 64, |plan, profile| {
        let mut source = FixtureNode::source(1);
        if let FixtureNode::Source { fail_prepare, .. } = &mut source {
            *fail_prepare = true;
        }
        let (_, source_start) = source.counts();
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let (_, sink_start) = sink.counts();
        let result = start_executor(&plan, profile, source, sink, policy(100, 1_000));
        assert_eq!(result.err(), Some(SchedulerError::PrepareFailed));
        assert_eq!(source_start.get(), 0);
        assert_eq!(sink_start.get(), 0);
    });
}

#[test]
fn full_queue_wakes_blocked_producer_after_consume_and_drains() {
    with_plan(1, 64, |plan, profile| {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut sink = FixtureNode::sink(seen.clone());
        if let FixtureNode::Sink {
            yields_remaining, ..
        } = &mut sink
        {
            *yields_remaining = 3;
        }
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(8),
            sink,
            policy(1_000, 10_000),
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen.borrow(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(executor.max_cord_occupancy(), 1);
        assert_eq!(
            executor.cord_occupancy(0),
            Some((0, 0, FlowQueueState::Completed))
        );
        let kinds = executor
            .events()
            .map(|event| event.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&SchedulerEventKind::Cord(
            conduit_core::FlowEventKind::PressureEntered
        )));
        assert!(kinds.contains(&SchedulerEventKind::Cord(
            conduit_core::FlowEventKind::ProducerReady
        )));
    });
}

#[test]
fn coupled_fanout_admits_all_branches_atomically_under_slow_pressure() {
    with_plan(1, 64, |base, profile| {
        let sink_b = ResolvedPlanNode {
            instance: InstancePath::new("root/sink-b").unwrap(),
            ..base.nodes[1]
        };
        let nodes = [base.nodes[0], base.nodes[1], sink_b];
        let cords = [
            ResolvedPlanCord {
                id: Id("branch/a"),
                ..base.cords[0]
            },
            ResolvedPlanCord {
                id: Id("branch/b"),
                to: ResolvedPlanPort {
                    node: sink_b.instance,
                    ..base.cords[0].to
                },
                ..base.cords[0]
            },
        ];
        let branches = [cords[0].id, cords[1].id];
        let fanouts = [PlanFanOut {
            id: Id("fanout/coupled"),
            producer: cords[0].from,
            mode: FanOutMode::Coupled,
            branches: &branches,
            duplicator: None,
            duplicator_input: None,
            duplication: DuplicationRule::Copy(pin("fixture/copy", 31)),
        }];
        let mut plan = ExecutionPlan {
            schema_version: 4,
            identity: ZERO,
            budget: PlanResourceBudget {
                cpu_units: 3,
                timers: 3,
                ..base.budget
            },
            nodes: &nodes,
            cords: &cords,
            fanouts: &fanouts,
            ..base
        };
        plan.identity = plan.semantic_hash(&mut [ZERO; 16]).unwrap();

        let seen_a = Rc::new(RefCell::new(Vec::new()));
        let seen_b = Rc::new(RefCell::new(Vec::new()));
        let mut slow = FixtureNode::sink_on(seen_b.clone(), 1);
        if let FixtureNode::Sink {
            yields_remaining, ..
        } = &mut slow
        {
            *yields_remaining = 3;
        }
        let scheduled = vec![
            ScheduledNode {
                driver: FixtureNode::CoupledSource {
                    next: 0,
                    total: 8,
                    prepare_count: Rc::new(Cell::new(0)),
                    start_count: Rc::new(Cell::new(0)),
                },
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: FixtureNode::sink_on(seen_a.clone(), 0),
                machine: machine(profile, &plan.nodes[1]),
            },
            ScheduledNode {
                driver: slow,
                machine: machine(profile, &plan.nodes[2]),
            },
        ];
        let mut executor = DeterministicExecutor::start(
            &plan,
            PlanValidationContext {
                supported_schema_version: 4,
                now: AuthorityTime {
                    basis: Id("clock/monotonic"),
                    tick: 2,
                },
            },
            policy(1_000, 10_000),
            reservation(),
            scheduled,
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen_a.borrow(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(&*seen_b.borrow(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(executor.max_cord_occupancy(), 1);
        assert!(executor.events().any(|event| {
            event.subject == conduit_runtime::SchedulerSubject::Cord(1)
                && event.kind
                    == SchedulerEventKind::Cord(conduit_core::FlowEventKind::PressureEntered)
        }));
    });
}

#[test]
fn staged_transaction_rolls_back_while_pending() {
    with_plan(1, 64, |plan, profile| {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut sink = FixtureNode::sink(seen.clone());
        if let FixtureNode::Sink { rollback_once, .. } = &mut sink {
            *rollback_once = true;
        }
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(1),
            sink,
            policy(100, 1_000),
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen.borrow(), &[0]);
        assert!(executor.events().any(|event| {
            event.kind
                == SchedulerEventKind::NodeOutcome {
                    outcome: conduit_core::StepOutcomeKind::Pending,
                }
        }));
    });
}

#[test]
fn bounded_fragments_preserve_zero_copy_handle_identity() {
    with_plan(2, 128, |plan, profile| {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut source = FixtureNode::fragmented_source(3);
        if let FixtureNode::Source { next, total, .. } = &mut source {
            *next = u64::MAX - 3;
            *total = u64::MAX;
        }
        let mut executor = start_executor(
            &plan,
            profile,
            source,
            FixtureNode::sink(seen.clone()),
            policy(100, 2_000),
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen.borrow(), &[u64::MAX - 3, u64::MAX - 2, u64::MAX - 1]);
        assert!(executor.max_cord_occupancy() <= 2);
    });
}

#[test]
fn two_input_join_commits_only_when_both_leases_are_ready() {
    with_plan(1, 64, |base, profile| {
        let source_a = base.nodes[0];
        let source_b = ResolvedPlanNode {
            instance: InstancePath::new("root/source-b").unwrap(),
            ..source_a
        };
        let sink = base.nodes[1];
        let nodes = [source_a, source_b, sink];
        let left = ResolvedPlanCord {
            from: ResolvedPlanPort {
                node: nodes[0].instance,
                port: Id("out"),
                ..base.cords[0].from
            },
            to: ResolvedPlanPort {
                node: nodes[2].instance,
                port: Id("left"),
                ..base.cords[0].to
            },
            id: Id("left-values"),
            ..base.cords[0]
        };
        let right = ResolvedPlanCord {
            from: ResolvedPlanPort {
                node: nodes[1].instance,
                port: Id("out"),
                ..base.cords[0].from
            },
            to: ResolvedPlanPort {
                node: nodes[2].instance,
                port: Id("right"),
                ..base.cords[0].to
            },
            id: Id("right-values"),
            ..base.cords[0]
        };
        let cords = [left, right];
        let mut plan = ExecutionPlan {
            identity: ZERO,
            budget: PlanResourceBudget {
                cpu_units: 3,
                timers: 3,
                ..base.budget
            },
            nodes: &nodes,
            cords: &cords,
            ..base
        };
        let mut scratch = [ZERO; 32];
        plan.identity = plan.semantic_hash(&mut scratch).unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let scheduled = vec![
            ScheduledNode {
                driver: FixtureNode::source_on(4, 0),
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: FixtureNode::source_on(4, 1),
                machine: machine(profile, &plan.nodes[1]),
            },
            ScheduledNode {
                driver: FixtureNode::Join {
                    seen: seen.clone(),
                    prepare_count: Rc::new(Cell::new(0)),
                    start_count: Rc::new(Cell::new(0)),
                },
                machine: machine(profile, &plan.nodes[2]),
            },
        ];
        let mut executor = DeterministicExecutor::start(
            &plan,
            PlanValidationContext {
                supported_schema_version: 3,
                now: AuthorityTime {
                    basis: Id("clock/monotonic"),
                    tick: 2,
                },
            },
            policy(1_000, 10_000),
            reservation(),
            scheduled,
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen.borrow(), &[(0, 0), (1, 1), (2, 2), (3, 3)]);
        assert!(executor.max_cord_occupancy() <= 1);
    });
}

#[test]
fn round_robin_prevents_a_ready_node_from_monopolizing() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 3,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 3,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut executor =
            start_executor(&plan, profile, source, sink, policy(100, 1_000)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        let decisions = executor
            .events()
            .filter_map(|event| match event.kind {
                SchedulerEventKind::Decision { .. } => Some(event.subject),
                _ => None,
            })
            .take(6)
            .collect::<Vec<_>>();
        assert_eq!(
            decisions,
            [
                conduit_runtime::SchedulerSubject::Node(0),
                conduit_runtime::SchedulerSubject::Node(1),
                conduit_runtime::SchedulerSubject::Node(0),
                conduit_runtime::SchedulerSubject::Node(1),
                conduit_runtime::SchedulerSubject::Node(0),
                conduit_runtime::SchedulerSubject::Node(1),
            ]
        );
        assert!(executor.events().any(|event| {
            matches!(
                event.kind,
                SchedulerEventKind::Decision {
                    reason: SchedulerDecisionReason::Progress
                }
            )
        }));
    });
}

#[test]
fn cancellation_wakes_a_blocked_producer_without_hidden_work() {
    with_plan(1, 64, |plan, profile| {
        let mut sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        if let FixtureNode::Sink {
            yields_remaining, ..
        } = &mut sink
        {
            *yields_remaining = 8;
        }
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(100),
            sink,
            policy(100, 2_000),
        )
        .unwrap();
        for _ in 0..6 {
            executor.run_one().unwrap();
        }
        assert_eq!(executor.cord_occupancy(0).unwrap().0, 1);
        executor.cancel(StopPolicy::Abort).unwrap();
        assert_eq!(executor.status(), SchedulerStatus::Cancelled);
        assert_eq!(executor.cord_occupancy(0).unwrap().0, 0);
        assert!(executor.events().any(|event| {
            matches!(
                event.kind,
                SchedulerEventKind::CancellationRequested {
                    stop: StopPolicy::Abort
                }
            )
        }));
    });
}

#[test]
fn drain_cancellation_preserves_accepted_values_and_terminates() {
    with_plan(2, 128, |plan, profile| {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut sink = FixtureNode::sink(seen.clone());
        if let FixtureNode::Sink {
            yields_remaining, ..
        } = &mut sink
        {
            *yields_remaining = 3;
        }
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(100),
            sink,
            policy(200, 4_000),
        )
        .unwrap();
        for _ in 0..5 {
            executor.run_one().unwrap();
        }
        let accepted = executor.cord_occupancy(0).unwrap().0;
        assert!(accepted > 0);
        executor.cancel(StopPolicy::Drain).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Cancelled
        );
        assert_eq!(
            executor.cord_occupancy(0),
            Some((0, 0, FlowQueueState::Cancelled))
        );
        assert!(seen.borrow().len() >= usize::from(accepted));
    });
}

#[test]
fn exact_host_operation_wakeup_and_node_failure_are_terminal() {
    with_plan(1, 64, |plan, profile| {
        let host = FixtureNode::HostWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut executor = start_executor(&plan, profile, host, sink, policy(100, 2_000)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Stalled
        );
        executor
            .notify_host_operation(Id("operation/device"))
            .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );

        let failing = FixtureNode::Fail {
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut executor =
            start_executor(&plan, profile, failing, sink, policy(100, 2_000)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap_err(),
            SchedulerError::NodeFailed
        );
        assert_eq!(
            executor.status(),
            SchedulerStatus::Failed(SchedulerError::NodeFailed)
        );
        assert_eq!(
            executor.cord_occupancy(0).unwrap().2,
            FlowQueueState::Failed
        );
    });
}

#[test]
fn repeated_zero_progress_yields_fail_at_the_exact_bound() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::YieldForever {
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 100,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut scheduler_policy = policy(100, 2_000);
        scheduler_policy.max_consecutive_yields = 3;
        let mut executor = start_executor(&plan, profile, source, sink, scheduler_policy).unwrap();
        let error = executor.run_until_stalled().unwrap_err();
        assert_eq!(error, SchedulerError::ZeroProgressLivelock);
        assert_eq!(
            executor.status(),
            SchedulerStatus::Failed(SchedulerError::ZeroProgressLivelock)
        );
    });
}

#[test]
fn decision_and_evidence_limits_fail_closed() {
    with_plan(1, 64, |plan, profile| {
        let progress = || FixtureNode::HostProgress {
            remaining: 100,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut executor =
            start_executor(&plan, profile, progress(), progress(), policy(3, 1_000)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap_err(),
            SchedulerError::DecisionLimitExceeded
        );

        let mut executor =
            start_executor(&plan, profile, progress(), progress(), policy(100, 16)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap_err(),
            SchedulerError::EvidenceCapacityExceeded
        );
        assert_eq!(
            executor.status(),
            SchedulerStatus::Failed(SchedulerError::EvidenceCapacityExceeded)
        );
    });
}

#[test]
fn scheduler_observations_become_bounded_execution_events_on_resonance() {
    with_plan(1, 64, |plan, profile| {
        let resource = ResourceRef {
            kind: Id("fixture/device"),
            id: Id("fixture/device-a"),
        };
        let effect = EffectRequirement {
            id: Id("read"),
            action: Id("fixture/read"),
            resource: ResourceSelector::Exact(resource),
            requester: plan.nodes[0].instance,
            audience: Id("fixture/run"),
            constraints: &[],
            check_at_use: true,
        };
        let capability = HostCapability {
            id: Id("fixture/capability"),
            action: effect.action,
            resource,
            host: plan.nodes[0].host,
            time_basis: Id("clock/monotonic"),
            observed_at_tick: 0,
            valid_until_tick: 1_000,
        };
        let grant = AuthorityGrant {
            id: Id("fixture/grant"),
            action: effect.action,
            resource,
            scope: AuthorityScope {
                root: effect.requester,
                descendants: false,
            },
            audience: effect.audience,
            constraints: &[],
            time_basis: Id("clock/monotonic"),
            not_before_tick: 0,
            expires_at_tick: 1_000,
            issued_for_host: plan.nodes[0].host,
            delegation: DelegationPolicy::None,
            audit_id: Id("fixture/audit"),
            terminal_policy: StopPolicy::Abort,
        };
        let binding = resolve_authority(
            effect,
            plan.nodes[0].host,
            AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
            &[capability],
            &[ObservedGrant {
                grant,
                status: GrantStatus::Active,
            }],
        )
        .unwrap();
        let effect_hash = effect.semantic_hash().unwrap();
        let required_resources = [Id("fixture/source-device")];
        let required_effects = [effect_hash];
        let nodes = [
            ResolvedPlanNode {
                required_resources: &required_resources,
                required_effects: &required_effects,
                ..plan.nodes[0]
            },
            plan.nodes[1],
        ];
        let resources = [PlanResourceBinding {
            id: required_resources[0],
            node: nodes[0].instance,
            resource,
            host_observation: nodes[0].host_observation,
        }];
        let authorities = [PlanAuthority {
            node: nodes[0].instance,
            effect_hash,
            grant_hash: grant.semantic_hash().unwrap(),
            effect,
            capability,
            grant,
            binding,
        }];
        let members = [nodes[0].instance, nodes[1].instance];
        let exports = [
            PlanExportBinding {
                boundary_port: Id("out"),
                member: plan.nodes[0].instance,
                member_port: Id("out"),
                direction: Direction::Output,
            },
            PlanExportBinding {
                boundary_port: Id("in"),
                member: plan.nodes[1].instance,
                member_port: Id("in"),
                direction: Direction::Input,
            },
        ];
        let composites = [PlanCompositeMapping {
            instance: InstancePath::new("root").unwrap(),
            definition_hash: hash(40),
            members: &members,
            exports: &exports,
        }];
        let stream = PlanEventStream {
            publisher: plan.nodes[0].instance,
            contract: EventStreamContract {
                id: Id("stream/runtime"),
                event_class: EventClass::NormativeEvidence,
                payload_type: TypeContractRef {
                    contract_id: Id("conduit/runtime-observation"),
                    schema_version: 1,
                    semantic_hash: hash(0x23),
                },
                retention: RetentionPolicy::Ring {
                    maximum_events: 1_000,
                    maximum_bytes: 2_000_000,
                },
                subscriber_coupling: SubscriberCoupling::Coupled(plan.cords[0].flow),
                delivery: ReplayDelivery::AtLeastOnce,
                maximum_publishers: 1,
                maximum_subscribers: 2,
                maximum_pending_operations: 2,
                maximum_projection_bytes: 64_000,
                provider: pin("provider/evidence", 41),
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
                redaction: true,
                maximum_events: 1_000,
                maximum_bytes: 2_000_000,
                maximum_subscribers: 2,
                maximum_pending_operations: 2,
            },
            allocation: PlanResourceBudget {
                memory_bytes: 2_000_000,
                evidence_bytes: 2_000_000,
                ..PlanResourceBudget::ZERO
            },
        };
        let evidence_policy = RuntimeEvidencePolicy {
            schema_version: RUNTIME_EVIDENCE_POLICY_VERSION,
            mode: RuntimeEvidenceMode::Record,
            stream: Some(stream.contract.id),
            maximum_events: 1_000,
            maximum_bytes: 2_000_000,
            required_reserve_events: 1,
            required_reserve_bytes: 4_096,
            telemetry_period: 2,
            telemetry_offset: 0,
            gap_summary_bytes: 2_048,
        };
        let mut plan = ExecutionPlan {
            schema_version: 8,
            identity: ZERO,
            resources: &resources,
            nodes: &nodes,
            event_streams: std::slice::from_ref(&stream),
            runtime_evidence: Some(evidence_policy),
            authorities: &authorities,
            composites: &composites,
            ..plan
        };
        let mut scratch = [ZERO; 64];
        plan.identity = plan.semantic_hash(&mut scratch).unwrap();

        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(2),
            FixtureNode::sink(seen),
            policy(100, 2_000),
        )
        .unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert!(executor.events().any(|event| {
            event.processing_latency_ticks == 1
                && matches!(event.kind, SchedulerEventKind::NodeOutcome { .. })
        }));
        let observations = executor.events().copied().collect::<Vec<_>>();
        let events = record_scheduler_evidence(
            &plan,
            RuntimeEvidenceContext {
                run: Id("run/fixture"),
                recorder: Id("recorder/runtime"),
                observer: Id("observer/executor"),
                monotonic_basis: Id("clock/scheduler"),
                correlation: EventCorrelation {
                    request: Some(Id("request/fixture")),
                    correlation: Some(Id("correlation/fixture")),
                    ..EventCorrelation::default()
                },
            },
            &observations,
        )
        .unwrap();

        assert!(
            events
                .iter()
                .any(|event| event.detail == "runtime/run-started")
        );
        assert!(
            events
                .iter()
                .any(|event| event.detail == "runtime/value-accepted")
        );
        assert!(
            events
                .iter()
                .any(|event| event.detail == "runtime/pressure-entered")
        );
        assert!(
            events
                .iter()
                .any(|event| event.detail == "runtime/pressure-cleared")
        );
        assert!(events.iter().any(|event| {
            event.kind == "derivation" && !event.relations.derived_from.is_empty()
        }));
        assert!(
            events
                .iter()
                .any(|event| event.detail == "runtime/telemetry-summary")
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.terminality,
                    conduit_runtime::OwnedEventTerminality::Terminal { .. }
                ))
                .count(),
            1
        );
        assert!(matches!(
            events.last().unwrap().terminality,
            conduit_runtime::OwnedEventTerminality::Terminal { .. }
        ));
        assert!(events.iter().any(|event| {
            event.logical_template.as_deref() == Some("root") && event.subject.starts_with("root/")
        }));
        assert!(events.iter().any(|event| {
            event.detail == "runtime/resource-bound"
                && matches!(
                    &event.payload,
                    OwnedEventPayload::InlinePublic { bytes, .. }
                        if bytes == b"fixture/source-device"
                )
        }));
        assert!(events.iter().any(|event| {
            event.detail == "runtime/authority-bound.a0"
                && matches!(
                    &event.payload,
                    OwnedEventPayload::Redacted {
                        sensitivity,
                        reason,
                        ..
                    } if sensitivity == "secret" && reason == "authority/redacted"
                )
        }));
        assert!(
            events
                .iter()
                .filter(|event| {
                    !matches!(
                        event.detail.as_str(),
                        "runtime/resource-bound" | "runtime/authority-bound.a0"
                    )
                })
                .all(|event| match &event.payload {
                    OwnedEventPayload::InlinePublic { bytes, .. } => bytes.len() == 52,
                    _ => false,
                })
        );

        let first = &events[0];
        let mut derivations = [Id(""); 16];
        let core = first.as_event(&mut derivations).unwrap();
        let envelope = extend_execution_event(
            core,
            EvidencePolicy {
                max_inline_payload_bytes: 52,
                reveal_redacted_byte_length: false,
                reveal_redacted_item_count: false,
            },
            EvidenceStreamExtension {
                stream: stream.contract.id,
                producer: stream.publisher,
                payload_type_when_none: stream.contract.payload_type,
                provenance: Id("runtime/executor"),
                recording_authority: None,
                integrity: hash(42),
            },
        )
        .unwrap();
        assert_eq!(envelope.class, EventClass::NormativeEvidence);
        assert_eq!(envelope.event, core.event_id);
        assert_eq!(envelope.plan_epoch, plan.identity);

        let sampled_flow = FlowPolicy::new(
            plan.cords[0].flow.capacity,
            Pressure::Sample(SampleSchedule::new(2, 0).unwrap()),
            plan.cords[0].flow.watermarks,
        )
        .unwrap();
        let sampled_cords = [ResolvedPlanCord {
            flow: sampled_flow,
            ..plan.cords[0]
        }];
        let mut loss_plan = ExecutionPlan {
            identity: ZERO,
            cords: &sampled_cords,
            ..plan
        };
        loss_plan.identity = loss_plan.semantic_hash(&mut scratch).unwrap();
        let mut loss_executor = start_executor(
            &loss_plan,
            profile,
            FixtureNode::source(3),
            FixtureNode::sink(Rc::new(RefCell::new(Vec::new()))),
            policy(100, 2_000),
        )
        .unwrap();
        assert_eq!(
            loss_executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        let loss_observations = loss_executor.events().copied().collect::<Vec<_>>();
        let loss_events = record_scheduler_evidence(
            &loss_plan,
            RuntimeEvidenceContext {
                run: Id("run/loss"),
                recorder: Id("recorder/runtime"),
                observer: Id("observer/executor"),
                monotonic_basis: Id("clock/scheduler"),
                correlation: EventCorrelation::default(),
            },
            &loss_observations,
        )
        .unwrap();
        assert!(
            loss_events
                .iter()
                .any(|event| event.detail == "runtime/value-sampled-out")
        );
        assert!(matches!(
            loss_events.last().unwrap().terminality,
            conduit_runtime::OwnedEventTerminality::Terminal { ref class, .. }
                if class == "succeeded"
        ));
    });
}

#[test]
fn long_run_never_exceeds_plan_capacity_and_exposes_metrics() {
    with_plan(2, 128, |plan, profile| {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut executor = start_executor(
            &plan,
            profile,
            FixtureNode::source(10_000),
            FixtureNode::sink(seen.clone()),
            policy(50_000, 200_000),
        )
        .unwrap();
        let started = Instant::now();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        let elapsed = started.elapsed();
        assert_eq!(seen.borrow().len(), 10_000);
        assert!(executor.max_cord_occupancy() <= 2);
        assert!(executor.max_ready_depth() <= 2);
        assert!(executor.decisions() <= 50_000);
        eprintln!(
            "deterministic scheduler: {} decisions in {:?}, max_ready={}, max_occupancy={}",
            executor.decisions(),
            elapsed,
            executor.max_ready_depth(),
            executor.max_cord_occupancy()
        );
    });
}

#[test]
fn every_deterministic_scheduler_fixture_is_owned_here() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/bounded-scheduler-v1.json"
    ))
    .unwrap();
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["runner"] == "deterministic-scheduler")
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 24);
    assert!(ids.contains(&"exact-preallocation"));
    assert!(ids.contains(&"two-input-join-atomic"));
    assert!(ids.contains(&"long-run-capacity-invariant"));
    assert!(ids.contains(&"no-async-runtime-plan-field"));
}
