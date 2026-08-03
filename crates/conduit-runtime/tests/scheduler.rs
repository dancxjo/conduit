use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use conduit_core::{
    ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime, BlockingFairness,
    BoundednessProfile, CancellationGuarantee, DelegationPolicy, Direction, DuplicationRule,
    EFFECT_COMMIT_PROFILE_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION, EffectCommitProfile,
    EffectDiscontinuity, EffectIdempotency, EffectRequirement, EventClass, EventCorrelation,
    EventProviderCapabilities, EventStreamContract, EvidenceCursorStatus, EvidencePolicy,
    EvidenceStreamExtension, ExecutionLimits, ExecutionPlan, ExecutionProfile, FanOutMode,
    FeedbackBoundaryKind, FeedbackInitialization, FeedbackReplayGapPolicy, FeedbackTerminalPolicy,
    FlowCapacity, FlowPolicy, FlowQueueState, FlowWatermarks, ForeignRetention, GrantStatus,
    HostCapability, Id, ImplementationMachine, InstancePath, InstantiationContext, LifecycleUsage,
    MemoryAccounting, MemoryCategory, MemoryClaim, ObservedGrant, PinnedDescriptor, PlanArtifact,
    PlanAuthority, PlanCompositeMapping, PlanEventStream, PlanExportBinding, PlanFanOut,
    PlanFeedbackBoundary, PlanHostObservation, PlanResourceBinding, PlanResourceBudget,
    PlanValidationContext, Pressure, RESOURCE_LEASE_SCHEMA_VERSION,
    RUNTIME_EVIDENCE_POLICY_VERSION, ReadyQueueDiscipline, ReplayDelivery, ResolvedPlanCord,
    ResolvedPlanNode, ResolvedPlanPort, ResourceLeaseContract, ResourceRef, ResourceSelector,
    ResourceSharingMode, RetentionPolicy, RuntimeEvidenceMode, RuntimeEvidencePolicy,
    SCHEDULER_CONTRACT_VERSION, SampleSchedule, SchedulerDecisionReason, SchedulerPolicy,
    SemanticHash, Sensitivity, StopPolicy, SubscriberCoupling, TypeContractRef,
    UnknownCommitPolicy, ValueEnvelopePolicy, ValueEnvelopeReason, extend_execution_event,
    resolve_authority,
};
use conduit_runtime::{
    DeterministicExecutor, ExactEvidenceCommitReceipt, ExactEvidenceCommitRequest,
    ExactEvidenceDrainError, ExactEvidenceProvider, ExactEvidenceProviderBinding,
    ExactEvidenceUseAuthority, ExactRunIdentity, ExactRunSession, ExactRunSessionRegistry,
    ExactRunState, OwnedEventPayload, RetainedValueUsage, RuntimeError, RuntimeEvidenceContext,
    RuntimeTimestamp, RuntimeValue, RuntimeValueEnvelope, ScheduledNode, SchedulerError,
    SchedulerEventKind, SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep,
    SendStatus, StepIo, record_scheduler_evidence, validate_hosted_execution_plan,
    validate_runtime_value_for_cord,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 0,
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
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn profile() -> ExecutionProfile<'static> {
    let mut value = ExecutionProfile {
        id: Id("fixture/scheduler-profile"),
        schema_version: 0,
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
        boot_id: Id("host/a-boot"),
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
        schema_version: 0,
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
        authorities: &[],
        hazard_closure: None,
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        supervisions: &[],
        unresolved: &[],
    };
    let mut scratch = [ZERO; 16];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    test(plan, &profile);
}

fn machine<'a>(
    profile: &'a ExecutionProfile<'a>,
    node: &ResolvedPlanNode<'a>,
) -> ImplementationMachine {
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
        envelope: Box<RuntimeValueEnvelope>,
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
        seen_envelopes: Option<Rc<RefCell<Vec<RuntimeValueEnvelope>>>>,
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
    TimerWait {
        waiting: bool,
        done: bool,
        prepare_count: Rc<Cell<u32>>,
        start_count: Rc<Cell<u32>>,
    },
    CancellationWait {
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
            envelope: Box::new(RuntimeValueEnvelope::EMPTY),
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
            envelope: Box::new(RuntimeValueEnvelope::EMPTY),
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
            envelope: Box::new(RuntimeValueEnvelope::EMPTY),
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
            seen_envelopes: None,
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
            | Self::TimerWait {
                prepare_count,
                start_count,
                ..
            }
            | Self::CancellationWait {
                prepare_count,
                start_count,
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

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match self {
            Self::Source {
                next,
                total,
                cord,
                fragments_per_value,
                envelope,
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
                    envelope: **envelope,
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
                    envelope: RuntimeValueEnvelope::EMPTY,
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
                seen_envelopes,
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
                    if let Some(envelopes) = seen_envelopes {
                        envelopes.borrow_mut().push(value.envelope);
                    }
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
            Self::TimerWait { waiting, done, .. } => {
                if *done {
                    SchedulerStep::Completed
                } else if !*waiting {
                    *waiting = true;
                    io.wait_for_timer(Id("timer/device"), io.tick() + 3)
                        .unwrap();
                    SchedulerStep::Pending
                } else {
                    *done = true;
                    io.record_host_progress().unwrap();
                    SchedulerStep::Progress
                }
            }
            Self::CancellationWait { .. } => {
                io.wait_for_host_operation(Id("operation/cancellation-wait"))
                    .unwrap();
                SchedulerStep::Pending
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

#[derive(Clone)]
struct RetainedFixtureNode {
    inner: FixtureNode,
    usage: RetainedValueUsage,
}

impl SchedulerNode for RetainedFixtureNode {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        self.inner.prepare()
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        self.inner.start()
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        self.inner.step(io)
    }

    fn retained_value_usage(&self) -> RetainedValueUsage {
        self.usage
    }
}

fn start_executor<'a>(
    plan: &'a ExecutionPlan<'a>,
    profile: &'a ExecutionProfile<'a>,
    source: FixtureNode,
    sink: FixtureNode,
    scheduler_policy: SchedulerPolicy,
) -> Result<DeterministicExecutor<FixtureNode>, SchedulerError> {
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

fn session(executor: DeterministicExecutor<FixtureNode>) -> ExactRunSession<FixtureNode> {
    let registry =
        ExactRunSessionRegistry::new(1, reservation().available_runtime_memory_bytes).unwrap();
    ExactRunSession::new(
        registry
            .admit(reservation().available_runtime_memory_bytes)
            .unwrap(),
        ExactRunIdentity {
            plan_identity: hash(201),
            source_semantic_hash: hash(202),
            plan_epoch: 7,
            run_id: "fixture/persistent-run".to_owned(),
        },
        executor,
    )
}

#[derive(Default)]
struct RecordingExactEvidenceState {
    fail: bool,
    fail_after_commit_once: bool,
    authority: Option<ExactEvidenceUseAuthority>,
    receipt_fault: Option<ReceiptFault>,
    committed: Vec<Vec<conduit_runtime::ExactEvidenceRecord>>,
    receipts: BTreeMap<(u64, u64), ExactEvidenceCommitReceipt>,
}

#[derive(Clone, Copy)]
enum ReceiptFault {
    Plan,
    Epoch,
    Run,
    ProviderId,
    Provider,
    ArtifactId,
    Artifact,
    HostObservation,
    StoreKind,
    Store,
    StoreGeneration,
    Grant,
    Lease,
    LeaseEpoch,
    Cursor,
    Digest,
    CommitIdentity,
}

struct RecordingExactEvidenceProvider {
    binding: ExactEvidenceProviderBinding,
    state: Rc<RefCell<RecordingExactEvidenceState>>,
}

impl ExactEvidenceProvider for RecordingExactEvidenceProvider {
    fn binding(&self) -> &ExactEvidenceProviderBinding {
        &self.binding
    }

    fn observe_use_authority(
        &self,
        _run: &ExactRunIdentity,
    ) -> Result<ExactEvidenceUseAuthority, RuntimeError> {
        self.state.borrow().authority.clone().ok_or_else(|| {
            RuntimeError::new(
                "CND-EVC-902",
                "fixture evidence provider is unavailable at use time",
            )
        })
    }

    fn commit_exact_evidence(
        &mut self,
        request: &ExactEvidenceCommitRequest,
        records: &[conduit_runtime::ExactEvidenceRecord],
    ) -> Result<ExactEvidenceCommitReceipt, RuntimeError> {
        let mut state = self.state.borrow_mut();
        if state.fail {
            return Err(RuntimeError::new(
                "CND-EVC-900",
                "fixture provider failed before commit",
            ));
        }
        let key = (request.start_cursor, request.end_cursor);
        if let Some(receipt) = state.receipts.get(&key) {
            return Ok(receipt.clone());
        }
        state.committed.push(records.to_vec());
        let mut receipt = ExactEvidenceCommitReceipt::acknowledged(request);
        match state.receipt_fault {
            Some(ReceiptFault::Plan) => receipt.plan_identity = hash(220),
            Some(ReceiptFault::Epoch) => receipt.plan_epoch += 1,
            Some(ReceiptFault::Run) => receipt.run_id = "run/wrong".to_owned(),
            Some(ReceiptFault::ProviderId) => {
                receipt.provider_implementation_id = "provider/wrong".to_owned();
            }
            Some(ReceiptFault::Provider) => {
                receipt.provider_implementation_identity = hash(221);
            }
            Some(ReceiptFault::ArtifactId) => {
                receipt.provider_artifact_id = "artifact/wrong".to_owned();
            }
            Some(ReceiptFault::Artifact) => {
                receipt.provider_artifact_digest = ArtifactDigest::from_bytes([222; 32]);
            }
            Some(ReceiptFault::HostObservation) => {
                receipt.host_observation_id = "observation/wrong".to_owned();
            }
            Some(ReceiptFault::StoreKind) => {
                receipt.store_resource_kind = "store-kind/wrong".to_owned();
            }
            Some(ReceiptFault::Store) => receipt.store_resource_id = "store/wrong".to_owned(),
            Some(ReceiptFault::StoreGeneration) => receipt.store_generation += 1,
            Some(ReceiptFault::Grant) => receipt.grant_hash = hash(224),
            Some(ReceiptFault::Lease) => receipt.lease_id = "lease/wrong".to_owned(),
            Some(ReceiptFault::LeaseEpoch) => receipt.lease_epoch += 1,
            Some(ReceiptFault::Cursor) => receipt.end_cursor += 1,
            Some(ReceiptFault::Digest) => receipt.batch_digest = hash(223),
            Some(ReceiptFault::CommitIdentity) => receipt.provider_commit_identity = hash(225),
            None => {}
        }
        if state.receipt_fault.is_some() {
            return Ok(receipt);
        }
        state.receipts.insert(key, receipt.clone());
        if state.fail_after_commit_once {
            state.fail_after_commit_once = false;
            return Err(RuntimeError::new(
                "CND-EVC-901",
                "fixture provider crashed after commit before acknowledgement",
            ));
        }
        Ok(receipt)
    }
}

fn evidence_binding() -> ExactEvidenceProviderBinding {
    ExactEvidenceProviderBinding {
        implementation_id: "fixture/evidence-provider".to_owned(),
        implementation_identity: hash(210),
        artifact_id: "fixture/evidence-artifact".to_owned(),
        artifact_digest: ArtifactDigest::from_bytes([211; 32]),
        host_observation_id: "observation/evidence-host".to_owned(),
        store_resource_kind: "evidence-store".to_owned(),
        store_resource_id: "fixture/evidence-store".to_owned(),
        store_generation: 3,
        grant_hash: hash(212),
        time_basis: "clock/monotonic".to_owned(),
    }
}

fn evidence_authority() -> ExactEvidenceUseAuthority {
    ExactEvidenceUseAuthority {
        grant_hash: hash(212),
        grant_active: true,
        run_id: "fixture/persistent-run".to_owned(),
        plan_epoch: 7,
        host_observation_id: "observation/evidence-host".to_owned(),
        store_resource_kind: "evidence-store".to_owned(),
        store_resource_id: "fixture/evidence-store".to_owned(),
        store_generation: 3,
        lease_id: "fixture/evidence-lease".to_owned(),
        lease_epoch: 7,
        lease_available: true,
        time_basis: "clock/monotonic".to_owned(),
        validated_at_tick: 2,
        valid_until_tick: 100,
    }
}

fn session_with_evidence(
    executor: DeterministicExecutor<FixtureNode>,
) -> (
    ExactRunSession<FixtureNode>,
    Rc<RefCell<RecordingExactEvidenceState>>,
) {
    let registry =
        ExactRunSessionRegistry::new(1, reservation().available_runtime_memory_bytes).unwrap();
    let state = Rc::new(RefCell::new(RecordingExactEvidenceState::default()));
    state.borrow_mut().authority = Some(evidence_authority());
    let binding = evidence_binding();
    let provider = RecordingExactEvidenceProvider {
        binding: binding.clone(),
        state: Rc::clone(&state),
    };
    let session = ExactRunSession::new_with_evidence_provider(
        registry
            .admit(reservation().available_runtime_memory_bytes)
            .unwrap(),
        ExactRunIdentity {
            plan_identity: hash(201),
            source_semantic_hash: hash(202),
            plan_epoch: 7,
            run_id: "fixture/persistent-run".to_owned(),
        },
        executor,
        binding,
        Box::new(provider),
    )
    .unwrap();
    (session, state)
}

fn evidence_fixture_session(
    plan: &ExecutionPlan<'_>,
    profile: &ExecutionProfile<'_>,
) -> (
    ExactRunSession<FixtureNode>,
    Rc<RefCell<RecordingExactEvidenceState>>,
) {
    let source = FixtureNode::HostProgress {
        remaining: 24,
        prepare_count: Rc::new(Cell::new(0)),
        start_count: Rc::new(Cell::new(0)),
    };
    let sink = FixtureNode::HostProgress {
        remaining: 24,
        prepare_count: Rc::new(Cell::new(0)),
        start_count: Rc::new(Cell::new(0)),
    };
    session_with_evidence(start_executor(plan, profile, source, sink, policy(512, 16)).unwrap())
}

#[test]
fn exact_session_capacity_is_admitted_before_start_and_released_when_admission_ends() {
    let sessions = ExactRunSessionRegistry::new(1, 128).unwrap();
    let first = sessions.admit(128).unwrap();
    assert_eq!(sessions.active_sessions(), 1);
    assert_eq!(sessions.reserved_bytes(), 128);
    assert!(matches!(
        sessions.admit(1),
        Err(SchedulerError::AllocationUnavailable)
    ));
    drop(first);
    assert_eq!(sessions.active_sessions(), 0);
    assert_eq!(sessions.reserved_bytes(), 0);
    assert!(sessions.admit(128).is_ok());
}

#[test]
fn exact_session_terminal_finalization_releases_its_registry_reservation() {
    with_plan(1, 64, |plan, profile| {
        let sessions =
            ExactRunSessionRegistry::new(1, reservation().available_runtime_memory_bytes).unwrap();
        let source = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run = ExactRunSession::new(
            sessions
                .admit(reservation().available_runtime_memory_bytes)
                .unwrap(),
            ExactRunIdentity {
                plan_identity: hash(201),
                source_semantic_hash: hash(202),
                plan_epoch: 7,
                run_id: "fixture/persistent-run".to_owned(),
            },
            start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap(),
        );
        assert_eq!(sessions.active_sessions(), 1);
        assert_eq!(
            run.cancel(StopPolicy::Abort).unwrap().state,
            ExactRunState::Terminal(conduit_core::TerminalClass::Cancelled)
        );
        assert!(run.finalize().is_ok());
        assert_eq!(sessions.active_sessions(), 0);
        assert_eq!(sessions.reserved_bytes(), 0);
    });
}

#[test]
fn dropping_a_nonterminal_session_fails_its_registry_closed() {
    with_plan(1, 64, |plan, profile| {
        let sessions =
            ExactRunSessionRegistry::new(1, reservation().available_runtime_memory_bytes).unwrap();
        let source = FixtureNode::HostWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let run = ExactRunSession::new(
            sessions
                .admit(reservation().available_runtime_memory_bytes)
                .unwrap(),
            ExactRunIdentity {
                plan_identity: hash(201),
                source_semantic_hash: hash(202),
                plan_epoch: 7,
                run_id: "fixture/persistent-run".to_owned(),
            },
            start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap(),
        );
        drop(run);
        assert!(sessions.has_abandoned_live_session());
        assert!(matches!(
            sessions.admit(reservation().available_runtime_memory_bytes),
            Err(SchedulerError::AllocationUnavailable)
        ));
    });
}

#[test]
fn exact_session_waits_across_repeated_pumps_and_wakes_the_same_run() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        let identity = run.identity().clone();
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(run.state(), ExactRunState::Waiting);
        assert_eq!(
            run.notify_host_operation(Id("operation/wrong"))
                .unwrap()
                .state,
            ExactRunState::Waiting
        );
        let decisions = run.high_water().decisions;
        for _ in 0..100 {
            assert_eq!(run.pump(1).unwrap().state, ExactRunState::Waiting);
        }
        assert_eq!(run.high_water().decisions, decisions);
        assert_eq!(run.identity(), &identity);

        run.notify_host_operation(Id("operation/device")).unwrap();
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(
            run.state(),
            ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
        );
        assert_eq!(run.identity(), &identity);
    });
}

#[test]
fn exact_session_releases_only_acknowledged_event_prefixes_with_monotonic_cursors() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 48,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 48,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(512, 16)).unwrap());
        let mut cursor = 0;
        let mut observed = 0_u64;

        while run.state() == ExactRunState::Active {
            let pump = run.pump(1).unwrap();
            assert!(pump.event_cursor >= cursor);
            let batch = run.read_scheduler_events(cursor, 16).unwrap();
            assert_eq!(batch.status, EvidenceCursorStatus::Available);
            assert!(!batch.events.is_empty());
            assert_eq!(batch.events[0].sequence, cursor);
            cursor = batch.next_cursor;
            observed += u64::try_from(batch.events.len()).unwrap();
            run.acknowledge_scheduler_events_through(cursor).unwrap();
            assert_eq!(run.scheduler_event_count(), 0);
            assert_eq!(run.retained_event_cursor(), cursor);
        }

        assert!(
            observed > 16,
            "the run outlived its resident event capacity"
        );
        assert_eq!(run.scheduler_event_count(), 0);
        assert_eq!(run.retained_event_cursor(), cursor);
        assert_eq!(run.pump(1).unwrap().event_cursor, cursor);
        assert_eq!(
            run.read_scheduler_events(0, 16).unwrap().status,
            EvidenceCursorStatus::Gap { resume_at: cursor }
        );
        assert!(matches!(
            run.acknowledge_scheduler_events_through(cursor + 1),
            Err(SchedulerError::InvalidPolicy)
        ));
    });
}

#[test]
fn exact_evidence_drain_commits_before_releasing_the_resident_prefix() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 24,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 24,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let (mut run, evidence_provider) = session_with_evidence(
            start_executor(&plan, profile, source, sink, policy(512, 16)).unwrap(),
        );
        let mut cursor = 0;

        run.pump(1).unwrap();
        evidence_provider.borrow_mut().fail = true;
        assert!(matches!(
            run.drain_exact_evidence(cursor, 16),
            Err(ExactEvidenceDrainError::Provider(_))
        ));
        assert!(run.scheduler_event_count() > 0);
        assert_eq!(run.retained_event_cursor(), cursor);

        evidence_provider.borrow_mut().fail = false;
        while run.state() == ExactRunState::Active {
            let batch = run.drain_exact_evidence(cursor, 16).unwrap();
            assert_eq!(batch.status, EvidenceCursorStatus::Available);
            assert!(batch.next_cursor > cursor);
            cursor = batch.next_cursor;
            assert_eq!(run.scheduler_event_count(), 0);
            run.pump(1).unwrap();
        }

        if run.scheduler_event_count() != 0 {
            let batch = run.drain_exact_evidence(cursor, 16).unwrap();
            assert_eq!(batch.status, EvidenceCursorStatus::Available);
            cursor = batch.next_cursor;
        }

        assert!(
            evidence_provider
                .borrow()
                .committed
                .iter()
                .flatten()
                .any(|record| record.sequence == 0)
        );
        assert!(
            evidence_provider
                .borrow()
                .committed
                .iter()
                .flatten()
                .any(|record| record.terminal_cause.is_some())
        );
        assert_eq!(run.scheduler_event_count(), 0);
        assert_eq!(run.retained_event_cursor(), cursor);
    });
}

#[test]
fn exact_evidence_rejects_every_inexact_provider_receipt_before_reclamation() {
    let faults = [
        ReceiptFault::Plan,
        ReceiptFault::Epoch,
        ReceiptFault::Run,
        ReceiptFault::ProviderId,
        ReceiptFault::Provider,
        ReceiptFault::ArtifactId,
        ReceiptFault::Artifact,
        ReceiptFault::HostObservation,
        ReceiptFault::StoreKind,
        ReceiptFault::Store,
        ReceiptFault::StoreGeneration,
        ReceiptFault::Grant,
        ReceiptFault::Lease,
        ReceiptFault::LeaseEpoch,
        ReceiptFault::Cursor,
        ReceiptFault::Digest,
        ReceiptFault::CommitIdentity,
    ];
    with_plan(1, 64, |plan, profile| {
        for fault in faults {
            let (mut run, provider) = evidence_fixture_session(&plan, profile);
            run.pump(1).unwrap();
            let cursor = run.retained_event_cursor();
            provider.borrow_mut().receipt_fault = Some(fault);
            assert!(matches!(
                run.drain_exact_evidence(cursor, 16),
                Err(ExactEvidenceDrainError::Receipt(_))
            ));
            assert_eq!(run.retained_event_cursor(), cursor);
            assert!(run.scheduler_event_count() > 0);
        }
    });
}

#[test]
fn exact_evidence_rechecks_grant_lease_and_time_at_use() {
    with_plan(1, 64, |plan, profile| {
        let (mut run, provider) = evidence_fixture_session(&plan, profile);
        run.pump(1).unwrap();
        let cursor = run.retained_event_cursor();
        let mut observations = Vec::new();

        let mut revoked = evidence_authority();
        revoked.grant_active = false;
        observations.push(revoked);
        let mut wrong_grant = evidence_authority();
        wrong_grant.grant_hash = hash(230);
        observations.push(wrong_grant);
        let mut missing_lease = evidence_authority();
        missing_lease.lease_available = false;
        observations.push(missing_lease);
        let mut wrong_lease = evidence_authority();
        wrong_lease.lease_id.clear();
        observations.push(wrong_lease);
        let mut wrong_lease_epoch = evidence_authority();
        wrong_lease_epoch.lease_epoch += 1;
        observations.push(wrong_lease_epoch);
        let mut wrong_epoch = evidence_authority();
        wrong_epoch.plan_epoch += 1;
        observations.push(wrong_epoch);
        let mut wrong_run = evidence_authority();
        wrong_run.run_id = "fixture/wrong-run".to_owned();
        observations.push(wrong_run);
        let mut wrong_store = evidence_authority();
        wrong_store.store_generation += 1;
        observations.push(wrong_store);
        let mut wrong_store_id = evidence_authority();
        wrong_store_id.store_resource_id = "fixture/wrong-store".to_owned();
        observations.push(wrong_store_id);
        let mut wrong_store_kind = evidence_authority();
        wrong_store_kind.store_resource_kind = "wrong-store-kind".to_owned();
        observations.push(wrong_store_kind);
        let mut wrong_host_observation = evidence_authority();
        wrong_host_observation.host_observation_id = "observation/wrong".to_owned();
        observations.push(wrong_host_observation);
        let mut wrong_time_basis = evidence_authority();
        wrong_time_basis.time_basis = "clock/wrong".to_owned();
        observations.push(wrong_time_basis);
        let mut stale = evidence_authority();
        stale.validated_at_tick = stale.valid_until_tick;
        observations.push(stale);

        for authority in observations {
            provider.borrow_mut().authority = Some(authority);
            assert!(matches!(
                run.drain_exact_evidence(cursor, 16),
                Err(ExactEvidenceDrainError::Authority(_))
            ));
            assert_eq!(run.retained_event_cursor(), cursor);
        }
        provider.borrow_mut().authority = None;
        assert!(matches!(
            run.drain_exact_evidence(cursor, 16),
            Err(ExactEvidenceDrainError::Authority(_))
        ));
        assert_eq!(run.retained_event_cursor(), cursor);
    });
}

#[test]
fn exact_evidence_retry_reconciles_commit_before_runtime_ack_without_duplication() {
    with_plan(1, 64, |plan, profile| {
        let (mut run, provider) = evidence_fixture_session(&plan, profile);
        run.pump(1).unwrap();
        let cursor = run.retained_event_cursor();
        provider.borrow_mut().fail_after_commit_once = true;

        assert!(matches!(
            run.drain_exact_evidence(cursor, 16),
            Err(ExactEvidenceDrainError::Provider(_))
        ));
        assert_eq!(run.retained_event_cursor(), cursor);
        assert_eq!(provider.borrow().committed.len(), 1);
        assert_eq!(provider.borrow().receipts.len(), 1);

        let batch = run.drain_exact_evidence(cursor, 16).unwrap();
        assert!(batch.next_cursor > cursor);
        assert_eq!(run.retained_event_cursor(), batch.next_cursor);
        assert_eq!(provider.borrow().committed.len(), 1);
        assert_eq!(provider.borrow().receipts.len(), 1);
    });
}

#[test]
fn exact_evidence_without_a_plan_selected_provider_cannot_release_events() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 24,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 24,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(512, 16)).unwrap());
        run.pump(1).unwrap();
        let cursor = run.retained_event_cursor();
        assert!(matches!(
            run.drain_exact_evidence(cursor, 16),
            Err(ExactEvidenceDrainError::Provider(_))
        ));
        assert_eq!(run.retained_event_cursor(), cursor);
    });
}

#[test]
fn exact_evidence_reads_are_bounded_and_do_not_acknowledge_their_source_events() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(512, 16)).unwrap());

        run.pump(1).unwrap();
        let before = run.scheduler_event_count();
        let first = run.read_exact_evidence(0, 1).unwrap();
        assert_eq!(first.status, EvidenceCursorStatus::Available);
        assert_eq!(first.records.len(), 1);
        assert_eq!(first.records[0].sequence, 0);
        assert_eq!(run.scheduler_event_count(), before);

        let repeated = run.read_exact_evidence(0, 1).unwrap();
        assert_eq!(repeated, first);
        let next = run.read_exact_evidence(first.next_cursor, 1).unwrap();
        assert_eq!(next.status, EvidenceCursorStatus::Available);
        assert!(next.next_cursor > first.next_cursor);
        assert!(
            next.records
                .iter()
                .all(|record| record.sequence >= first.next_cursor)
        );
        assert_eq!(run.scheduler_event_count(), before);
    });
}

#[test]
fn exact_session_timer_wake_resumes_the_same_waiting_epoch() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::TimerWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        let identity = run.identity().clone();
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(run.state(), ExactRunState::Waiting);
        let deadline = run.next_timer_deadline().expect("timer is retained");
        assert_eq!(
            run.advance_to(deadline).unwrap().state,
            ExactRunState::Active
        );
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(
            run.state(),
            ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
        );
        assert_eq!(run.identity(), &identity);
    });
}

#[test]
fn exact_session_pump_after_terminal_keeps_the_terminal_epoch_intact() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostProgress {
            remaining: 1,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 1,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(
            run.state(),
            ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
        );
        let identity = run.identity().clone();
        let decisions = run.high_water().decisions;
        let evidence = run.exact_evidence();

        assert_eq!(
            run.pump(1).unwrap().state,
            ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
        );
        assert_eq!(run.identity(), &identity);
        assert_eq!(run.high_water().decisions, decisions);
        assert_eq!(run.exact_evidence(), evidence);
    });
}

#[test]
fn exact_session_rejects_nonterminal_finalization_without_dropping_the_epoch() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        assert!(matches!(run.finalize(), Err(ExactRunState::Active)));
        assert_eq!(run.identity().plan_epoch, 7);
        assert_eq!(run.pump(1).unwrap().state, ExactRunState::Active);
    });
}

#[test]
fn exact_session_pump_quantum_and_cancellation_keep_one_epoch() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::HostWait {
            waiting: false,
            done: false,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::sink(Rc::new(RefCell::new(Vec::new())));
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        let first = run.pump(1).unwrap();
        assert_eq!(first.state, ExactRunState::Active);
        assert_eq!(first.decisions, 1);
        assert_eq!(run.identity().plan_epoch, 7);

        let drain = run.cancel(StopPolicy::Drain).unwrap();
        assert!(matches!(
            drain.state,
            ExactRunState::Quiescing | ExactRunState::Terminal(_)
        ));
        while matches!(
            run.state(),
            ExactRunState::Active | ExactRunState::Quiescing
        ) {
            run.pump(1).unwrap();
        }
        assert_eq!(
            run.state(),
            ExactRunState::Terminal(conduit_core::TerminalClass::Cancelled)
        );

        let source = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::HostProgress {
            remaining: 8,
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut aborted =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        assert_eq!(
            aborted.cancel(StopPolicy::Abort).unwrap().state,
            ExactRunState::Terminal(conduit_core::TerminalClass::Cancelled)
        );
    });
}

#[test]
fn exact_session_drain_deadline_fails_closed_without_resetting_the_epoch() {
    with_plan(1, 64, |plan, profile| {
        let source = FixtureNode::CancellationWait {
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let sink = FixtureNode::CancellationWait {
            prepare_count: Rc::new(Cell::new(0)),
            start_count: Rc::new(Cell::new(0)),
        };
        let mut run =
            session(start_executor(&plan, profile, source, sink, policy(256, 2_000)).unwrap());
        let identity = run.identity().clone();
        while run.state() == ExactRunState::Active {
            run.pump(1).unwrap();
        }
        assert_eq!(run.state(), ExactRunState::Waiting);
        assert_eq!(
            run.cancel(StopPolicy::Drain).unwrap().state,
            ExactRunState::Quiescing
        );
        run.pump(1).unwrap();
        run.pump(1).unwrap();
        assert_eq!(run.state(), ExactRunState::Quiescing);
        let error = run.advance_to(64).unwrap_err();
        assert_eq!(error.code(), "CND-SCH-012");
        assert_eq!(run.identity(), &identity);
    });
}

#[test]
fn scheduler_preserves_only_plan_authorized_value_envelopes() {
    with_plan(2, 128, |base, profile| {
        let clocks = [Id("clock/monotonic")];
        let representation = pin("fixture/runtime-representation", 90);
        let policies = [ValueEnvelopePolicy {
            cord: base.cords[0].id,
            representation,
            maximum_payload_bytes: base.cords[0].flow.capacity.max_value_bytes(),
            maximum_envelope_bytes: 64,
            maximum_fragments: 4,
            maximum_fragment_bytes: 64,
            maximum_timestamps: 1,
            clock_domains: &clocks,
            identity_allowed: true,
            correlation_allowed: true,
            causation_allowed: true,
            provenance_allowed: true,
            sensitivity_ceiling: Sensitivity::Restricted,
        }];
        let cords = [ResolvedPlanCord {
            queue_memory_bytes: base.cords[0].queue_memory_bytes
                + u64::from(base.cords[0].flow.capacity.items())
                    * u64::from(policies[0].maximum_envelope_bytes),
            ..base.cords[0]
        }];
        let mut plan = ExecutionPlan {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            identity: ZERO,
            cords: &cords,
            value_envelopes: &policies,
            ..base
        };
        plan.identity = plan.semantic_hash(&mut [ZERO; 16]).unwrap();

        let mut timestamps = [RuntimeTimestamp::default(); conduit_core::MAX_VALUE_CLOCK_DOMAINS];
        timestamps[0] = RuntimeTimestamp {
            domain_index: 0,
            tick: 7,
            uncertainty_ticks: 1,
        };
        let envelope = RuntimeValueEnvelope {
            representation: representation.semantic_hash,
            envelope_bytes: 48,
            fragment_count: 1,
            fragment_bytes: 8,
            identity: Some(hash(91)),
            correlation: Some(hash(92)),
            causation: Some(hash(93)),
            provenance: Some(hash(94)),
            timestamp_count: 1,
            timestamps,
            sensitivity: Sensitivity::Restricted,
        };
        let value = RuntimeValue {
            handle: 1,
            accounted_bytes: 8,
            envelope,
        };
        validate_runtime_value_for_cord(&plan, cords[0].id, value).unwrap();

        let mut forbidden = value;
        forbidden.envelope.sensitivity = Sensitivity::Secret;
        assert_eq!(
            validate_runtime_value_for_cord(&plan, cords[0].id, forbidden),
            Err(ValueEnvelopeReason::SensitivityWidening)
        );
        assert_eq!(
            validate_runtime_value_for_cord(&base, base.cords[0].id, value),
            Err(ValueEnvelopeReason::UnauthorizedField)
        );

        let mut source = FixtureNode::source(1);
        if let FixtureNode::Source {
            envelope: source_envelope,
            ..
        } = &mut source
        {
            **source_envelope = envelope;
        }
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_envelopes = Rc::new(RefCell::new(Vec::new()));
        let mut sink = FixtureNode::sink(seen);
        if let FixtureNode::Sink {
            seen_envelopes: destination,
            ..
        } = &mut sink
        {
            *destination = Some(seen_envelopes.clone());
        }
        let mut executor =
            start_executor(&plan, profile, source, sink, policy(100, 1_000)).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(&*seen_envelopes.borrow(), &[envelope]);
    });
}

#[test]
fn feedback_cycle_requires_a_finite_boundary_and_reserves_retained_state() {
    with_plan(2, 128, |base, profile| {
        let cords = [
            base.cords[0],
            ResolvedPlanCord {
                id: Id("feedback"),
                from: ResolvedPlanPort {
                    node: base.nodes[1].instance,
                    ..base.cords[0].from
                },
                to: ResolvedPlanPort {
                    node: base.nodes[0].instance,
                    ..base.cords[0].to
                },
                ..base.cords[0]
            },
        ];
        let mut cyclic = ExecutionPlan {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            identity: ZERO,
            cords: &cords,
            ..base
        };
        cyclic.identity = cyclic.semantic_hash(&mut [ZERO; 16]).unwrap();
        let context = PlanValidationContext {
            supported_schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        };
        assert_eq!(
            validate_hosted_execution_plan(&cyclic, context)
                .unwrap_err()
                .code
                .as_str(),
            "CND-FBK-002"
        );

        let boundaries = [PlanFeedbackBoundary {
            id: Id("fixture/feedback-state"),
            node: base.nodes[1].instance,
            cord: cords[1].id,
            kind: FeedbackBoundaryKind::State,
            initialization: FeedbackInitialization::Empty,
            initial_items: 0,
            initial_bytes: 0,
            maximum_retained_items: 2,
            maximum_retained_bytes: 64,
            delay_ticks: 0,
            clock: None,
            replay_gap: FeedbackReplayGapPolicy::Fail,
            cancellation: pin("fixture/feedback-cancellation", 96),
            terminal: FeedbackTerminalPolicy::DropRetained,
        }];
        let mut admitted = ExecutionPlan {
            identity: ZERO,
            feedback_boundaries: &boundaries,
            ..cyclic
        };
        admitted.identity = admitted.semantic_hash(&mut [ZERO; 16]).unwrap();
        validate_hosted_execution_plan(&admitted, context).unwrap();

        let mut executor = start_executor(
            &admitted,
            profile,
            FixtureNode::source(1),
            FixtureNode::sink(Rc::new(RefCell::new(Vec::new()))),
            policy(100, 1_000),
        )
        .unwrap();
        assert_eq!(executor.allocation().feedback_memory_bytes, 64);
        executor.cancel(StopPolicy::Abort).unwrap();
        assert_eq!(
            executor.run_until_stalled().unwrap(),
            SchedulerStatus::Cancelled
        );
    });
}

#[test]
fn implementation_retained_values_are_enforced_after_each_step() {
    with_plan(2, 128, |plan, profile| {
        let nodes = vec![
            ScheduledNode {
                driver: RetainedFixtureNode {
                    inner: FixtureNode::source(1),
                    usage: RetainedValueUsage {
                        values: 1,
                        bytes: 8,
                    },
                },
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: RetainedFixtureNode {
                    inner: FixtureNode::sink(Rc::new(RefCell::new(Vec::new()))),
                    usage: RetainedValueUsage::default(),
                },
                machine: machine(profile, &plan.nodes[1]),
            },
        ];
        let mut executor = DeterministicExecutor::start(
            &plan,
            PlanValidationContext {
                supported_schema_version: 0,
                now: AuthorityTime {
                    basis: Id("clock/monotonic"),
                    tick: 2,
                },
            },
            policy(100, 1_000),
            reservation(),
            nodes,
        )
        .unwrap();

        assert_eq!(
            executor.run_until_stalled().unwrap_err(),
            SchedulerError::StepContractViolation
        );
    });
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
                supported_schema_version: 0,
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
        assert_eq!(allocation.node_memory_bytes, 1_024);
        assert_eq!(allocation.cord_memory_bytes, 128);
        assert_eq!(allocation.pool_memory_bytes, 0);
        assert_eq!(allocation.event_stream_memory_bytes, 0);
        assert_eq!(allocation.job_memory_bytes, 0);
        assert_eq!(allocation.planned_memory_bytes, 1_152);
        assert_eq!(allocation.queue_payload_bytes, 128);
        assert_eq!(allocation.queue_slots, 2);
        assert_eq!(allocation.ready_slots, 2);
        assert!(allocation.executor_overhead_bytes > 0);
        assert!(allocation.scheduler_evidence_bytes > 0);
        assert_eq!(executor.policy(), policy(100, 1_000));
        assert!(allocation.planned_memory_bytes + allocation.executor_overhead_bytes <= 32_000_000);
        assert_eq!(executor.high_water().queue_items, 0);
        assert_eq!(executor.high_water().queue_payload_bytes, 0);

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
            schema_version: 0,
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
                supported_schema_version: 0,
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
        let high_water = executor.high_water();
        assert!(high_water.queue_items <= 2);
        assert!(high_water.queue_payload_bytes <= plan.cords[0].queue_memory_bytes);
        assert_eq!(high_water.decisions, executor.decisions());
        assert_eq!(
            usize::try_from(high_water.event_slots).unwrap(),
            executor.event_count()
        );
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
                supported_schema_version: 0,
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
            administrative_class: None,
            policy_budget_class: None,
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
        let lease = ResourceLeaseContract {
            schema_version: RESOURCE_LEASE_SCHEMA_VERSION,
            id: Id("fixture/source-lease"),
            resource_binding: required_resources[0],
            holder: effect.requester,
            run: effect.audience,
            epoch: 1,
            scope: Id("fixture/read-scope"),
            sharing: ResourceSharingMode::Exclusive,
            reservation: PlanResourceBudget {
                memory_bytes: 1,
                ..PlanResourceBudget::ZERO
            },
            time_basis: capability.time_basis,
            issued_at_tick: 0,
            expires_at_tick: 1_000,
            revocation_grace_ticks: 1,
            cleanup_ticks: 2,
            maximum_operations: 2,
            maximum_evidence_events: 4,
            cleanup_escalation: pin("fixture/force-close", 42),
            foreign_retention: ForeignRetention::Unsupported,
        };
        let commit_profile = EffectCommitProfile {
            schema_version: EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
            id: Id("fixture/read-commit"),
            operation: effect.action,
            resource_lease: lease.id,
            commit_boundary: pin("fixture/read-commit-boundary", 43),
            idempotency: EffectIdempotency::ReconcileBeforeRetry,
            unknown_commit: UnknownCommitPolicy::Reconcile,
            discontinuity: EffectDiscontinuity::ReconcileRequired,
            cleanup: pin("fixture/read-cleanup", 44),
            maximum_attempts: 2,
            evidence_events_per_attempt: 2,
        };
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
            lease: Some(lease),
        }];
        let authorities = [PlanAuthority {
            node: nodes[0].instance,
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
                    schema_version: 0,
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
            schema_version: 0,
            identity: ZERO,
            resources: &resources,
            nodes: &nodes,
            event_streams: std::slice::from_ref(&stream),
            runtime_evidence: Some(evidence_policy),
            evidence_provider: None,
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
        "../../../conformance/c4/bounded-scheduler.json"
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

#[test]
fn every_persistent_exact_run_fixture_is_owned_by_the_session_boundary() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/persistent-exact-run.json"
    ))
    .unwrap();
    assert_eq!(fixture["suite"], "conduit.persistent-exact-run");
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 18);
    for id in [
        "live-waiting-repeated-pump",
        "timer-wake-resumes-session",
        "host-operation-wake-resumes-session",
        "wrong-wake-does-not-resume",
        "drain-cancels-active-session",
        "abort-cancels-active-session",
        "abort-waits-for-exact-provider-cleanup",
        "provider-cleanup-deadline-fails-same-epoch",
        "finalize-before-terminal-rejected",
        "source-edit-cannot-mutate-active-epoch",
    ] {
        assert!(ids.contains(&id), "fixture must name `{id}`");
    }
}
