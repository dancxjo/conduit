use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    collections::BTreeSet,
    fmt::Write as _,
    hint::black_box,
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use bumpalo::Bump;
use clap::{Parser, ValueEnum};
use conduit_compile::{InstalledHostObservationInput, InstalledProfile, compile_source};
use conduit_core::{
    ArtifactDigest, AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee,
    CompatibilityOutcome, Direction, DuplicationRule, EvidenceCursorStatus, ExecutionLimits,
    ExecutionPlan, ExecutionProfile, FanOutMode, FlowCapacity, FlowPolicy, FlowQueueState,
    FlowTypeFacts, FlowWatermarks, Id, ImplementationMachine, InstancePath, InstantiationContext,
    LifecycleUsage, MemoryAccounting, MemoryCategory, MemoryClaim, PinnedDescriptor, PlanArtifact,
    PlanFanOut, PlanHostObservation, PlanResourceBudget, PlanValidationContext, Pressure,
    ReadyQueueDiscipline, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort,
    SCHEDULER_CONTRACT_VERSION, SampleSchedule, SchedulerPolicy, SemanticHash, Sensitivity,
    StopPolicy, TraitProof, TypeContractRef, WatchAdmission, WatchRetention, WatchSubject,
    validate_execution_plan,
};
use conduit_runtime::{
    DeterministicExecutor, ExactRunContext, ExactRunIdentity, ExactRunIo, ExactRunSession,
    ExactRunSessionRegistry, ExactRunState, ExactWatchMaterial, ExactWatchOperation,
    ExactWatchUseAuthority, Registry, RetainedValueUsage, RuntimeValue, RuntimeValueEnvelope,
    ScheduledNode, SchedulerEventKind, SchedulerNode, SchedulerReservation, SchedulerStatus,
    SchedulerStep, SchedulerSubject, SendStatus, StepIo, hosted_service_use_observations,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

struct CountingAllocator;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the request is forwarded unchanged to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && MEASURING.load(Ordering::Relaxed) {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout came from the matching system allocation.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the request is forwarded unchanged to the system allocator.
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() && MEASURING.load(Ordering::Relaxed) {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("benchmark/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([31; 32]),
};
const CLAIMS: [MemoryClaim; 1] = [MemoryClaim {
    category: MemoryCategory::PortTransactions,
    accounting: MemoryAccounting::ExecutorAllocated,
    bytes: 512,
}];
const ISOLATED_CLAIMS: [MemoryClaim; 2] = [
    CLAIMS[0],
    MemoryClaim {
        category: MemoryCategory::Retained,
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
    max_input_bytes: 16,
    max_output_reservations: 2,
    max_output_bytes: 16,
    max_transactions: 1,
    max_fragments_per_step: 0,
    max_pending_operations: 0,
    max_timers: 0,
    max_child_tasks: 0,
    max_host_buffer_bytes: 0,
    max_foreign_queue_items: 0,
    max_foreign_queue_bytes: 0,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 512,
    cancellation_ticks: 8,
};

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Workload {
    Map,
    MapFilter,
    Merge,
    BoundedAsync,
    Overload,
    Fanout,
    SharedPayloadFanout,
    PersistentWake,
    PersistentTimer,
}

impl Workload {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::MapFilter => "map-filter",
            Self::Merge => "merge",
            Self::BoundedAsync => "bounded-async",
            Self::Overload => "overload",
            Self::Fanout => "fanout",
            Self::SharedPayloadFanout => "shared-payload-fanout",
            Self::PersistentWake => "persistent-wake",
            Self::PersistentTimer => "persistent-timer",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum PressurePolicy {
    Block,
    Reject,
    Coalesce,
    Sample,
    DropDisposable,
    Disconnect,
    Fail,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum SlowBranches {
    One,
    All,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum FanoutPublication {
    Coupled,
    Isolated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum PayloadBinding {
    SharedHandle,
    BranchLocalUppercaseCopy,
}

impl PayloadBinding {
    const fn representation(self) -> &'static str {
        match self {
            Self::SharedHandle => "hosted-generation-safe-shared-text-handle",
            Self::BranchLocalUppercaseCopy => "hosted-branch-local-uppercase-copy",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum TerminationRequest {
    Complete,
    Drain,
    Abort,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum ConsumerPattern {
    Sustained,
    Bursty,
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum SessionMode {
    Finite,
    Persistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WakeMode {
    None,
    HostOperation,
    Timer(u64),
}

impl SessionMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Finite => "finite-executor",
            Self::Persistent => "persistent-exact-run-session",
        }
    }
}

impl ConsumerPattern {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Sustained => "sustained-slow-then-recover",
            Self::Bursty => "bursty",
        }
    }
}

impl TerminationRequest {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Drain => "drain",
            Self::Abort => "abort",
        }
    }

    const fn stop_policy(self) -> Option<StopPolicy> {
        match self {
            Self::Complete => None,
            Self::Drain => Some(StopPolicy::Drain),
            Self::Abort => Some(StopPolicy::Abort),
        }
    }
}

impl FanoutPublication {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Coupled => "coupled",
            Self::Isolated => "isolated",
        }
    }
}

impl SlowBranches {
    const fn as_str(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::All => "all",
        }
    }
}

impl PressurePolicy {
    const fn id(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Reject => "reject",
            Self::Coalesce => "coalesce",
            Self::Sample => "sample",
            Self::DropDisposable => "drop-disposable",
            Self::Disconnect => "disconnect",
            Self::Fail => "fail",
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Reject => "reject",
            Self::Coalesce => "coalesce/latest-wins",
            Self::Sample => "sample/every-2-offset-0",
            Self::DropDisposable => "drop-disposable",
            Self::Disconnect => "disconnect",
            Self::Fail => "fail",
        }
    }

    const fn loss(self) -> &'static str {
        match self {
            Self::Block | Self::Reject | Self::Disconnect | Self::Fail => "none",
            Self::Coalesce => "replaced queued values are counted as coalesced",
            Self::Sample => "schedule exclusions are sampled; selected saturation loss is dropped",
            Self::DropDisposable => "type-proven disposable values are counted as dropped",
        }
    }
}

#[derive(Debug, Parser)]
#[command(about = "Run one repeatable Conduit reference-scheduler comparison sample")]
struct Args {
    #[arg(long, value_enum)]
    workload: Workload,
    #[arg(long, default_value_t = 1)]
    operators: usize,
    #[arg(long, default_value_t = 1_000_000)]
    values: u64,
    #[arg(long, default_value_t = 256)]
    queue_items: u16,
    #[arg(long, default_value_t = 1024)]
    latency_sample_stride: u64,
    #[arg(long, default_value_t = 2)]
    warmup_trials: u32,
    #[arg(long, default_value_t = 9)]
    measured_trials: u32,
    #[arg(long)]
    identity_loop: bool,
    #[arg(long, value_enum, default_value_t = PressurePolicy::Block)]
    pressure_policy: PressurePolicy,
    #[arg(long, default_value_t = 3)]
    slow_consumer_yields: u32,
    #[arg(long, default_value_t = 1)]
    fanout_branches: u16,
    #[arg(long, value_enum, default_value_t = FanoutPublication::Coupled)]
    fanout_mode: FanoutPublication,
    #[arg(long, value_enum, default_value_t = SlowBranches::One)]
    slow_branches: SlowBranches,
    #[arg(long, value_enum, default_value_t = TerminationRequest::Complete)]
    termination_request: TerminationRequest,
    #[arg(long, default_value_t = 0)]
    cancel_after_offers: u64,
    #[arg(long, value_enum, default_value_t = ConsumerPattern::Sustained)]
    consumer_pattern: ConsumerPattern,
    #[arg(long, default_value_t = 0)]
    consumer_burst_items: u64,
    #[arg(long, value_enum, default_value_t = SessionMode::Finite)]
    session_mode: SessionMode,
    #[arg(long, default_value_t = 0)]
    session_pump_quantum: u64,
    #[arg(long, default_value_t = 0)]
    residency_plateau_after_wakes: u64,
    #[arg(long, default_value_t = 0)]
    timer_advance_ticks: u64,
    #[arg(long, default_value_t = 0)]
    payload_bytes: u64,
    #[arg(long, value_enum, default_value_t = PayloadBinding::SharedHandle)]
    payload_binding: PayloadBinding,
    #[arg(long, default_value_t = 0)]
    watch_slots: u16,
    #[arg(long, default_value_t = 0)]
    watch_preview_bytes: u32,
}

#[derive(Serialize)]
struct PhaseTimes {
    assembly_ns: u64,
    plan_seal_ns: Option<u64>,
    start_ns: Option<u64>,
    steady_ns: u64,
    pressure_ns: Option<u64>,
    recovery_ns: Option<u64>,
    pressure_cycles: Option<u64>,
    recovery_cycles: Option<u64>,
}

#[derive(Serialize)]
struct ExecutionMeasurement {
    scheduler_decisions: Option<u64>,
    producer_stall_ns: Option<u64>,
    drain_ns: Option<u64>,
    abort_ns: Option<u64>,
    session_pumps: Option<u64>,
    session_reserved_bytes: Option<u64>,
    pressured_items_at_stop: Option<u64>,
    session_host_wakes: Option<u64>,
    session_timer_wakes: Option<u64>,
    residency_plateau_verified: Option<bool>,
    residency_checkpoint_queue_items_high_water: Option<u64>,
    residency_checkpoint_queue_payload_bytes_high_water: Option<u64>,
    residency_checkpoint_ready_slots_high_water: Option<u32>,
    residency_checkpoint_evidence_slots_high_water: Option<u32>,
    unique_value_handles: Option<u64>,
    branch_deliveries: Option<u64>,
    shared_handle_publications: Option<u64>,
    payload_copy_operations: Option<u64>,
    payload_bytes_copied: Option<u64>,
}

#[derive(Serialize)]
struct AllocationMeasurement {
    scope: &'static str,
    calls: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct MemoryMeasurement {
    resident_before_bytes: Option<u64>,
    resident_after_bytes: Option<u64>,
    resident_peak_bytes: Option<u64>,
    planned_memory_bytes: Option<u64>,
    executor_overhead_bytes: Option<u64>,
    queue_items_high_water: Option<u64>,
    queue_max_cord_items_high_water: Option<u16>,
    queue_payload_bytes_high_water: Option<u64>,
    ready_slots_high_water: Option<u32>,
    evidence_slots_high_water: Option<u32>,
    value_resident_slots_after_terminal: Option<u32>,
    value_resident_bytes_after_terminal: Option<u64>,
    value_slots_high_water: Option<u32>,
    value_bytes_high_water: Option<u64>,
    value_slots_capacity: Option<u32>,
    value_bytes_capacity: Option<u64>,
    host_io_capacity_bytes: Option<u64>,
    host_io_output_bytes: Option<u64>,
    watch_admitted_slots: Option<u32>,
    watch_attached_slots: Option<u32>,
    watch_retained_observations: Option<u64>,
    watch_retained_preview_bytes: Option<u64>,
    watch_dropped_observations: Option<u64>,
    watch_maximum_observations: Option<u64>,
    watch_maximum_preview_bytes: Option<u64>,
}

#[derive(Serialize)]
struct LatencyMeasurement {
    clock: &'static str,
    sample_stride: u64,
    samples_ns: Vec<u64>,
}

#[derive(Serialize)]
struct RuntimeConfiguration {
    id: &'static str,
    comparison_role: &'static str,
    execution_mode: &'static str,
    build_profile: &'static str,
    scheduler: &'static str,
    fusion: &'static str,
    batching: &'static str,
    concurrency: u32,
}

#[derive(Serialize)]
struct WorkloadIdentity {
    id: Workload,
    operators: usize,
    input_values: u64,
    queue_capacity_items: u16,
    ordering: &'static str,
    pressure: &'static str,
    terminal: &'static str,
    loss: &'static str,
    slow_consumer_yields: u32,
    recovery_after_outputs: u64,
    fanout_branches: u16,
    fanout_mode: &'static str,
    slow_branches: &'static str,
    termination_request: &'static str,
    cancel_after_offers: u64,
    consumer_pattern: &'static str,
    consumer_burst_items: u64,
    session_mode: &'static str,
    session_pump_quantum: u64,
    residency_plateau_after_wakes: u64,
    timer_advance_ticks: u64,
    payload_bytes: u64,
    payload_representation: &'static str,
    watch_slots: u16,
    watch_preview_bytes: u32,
    watch_retention: &'static str,
}

#[derive(Serialize)]
struct ExactIdentity {
    logical_fixture: String,
    plan_identity: Option<String>,
    source_semantic_hash: Option<String>,
    artifact_digest: Option<String>,
}

#[derive(Serialize)]
struct OutcomeMeasurement {
    offered: u64,
    admitted: u64,
    completed_useful: u64,
    rejected: u64,
    sampled: u64,
    coalesced: u64,
    dropped: u64,
    cancelled: u64,
    retried: u64,
    terminal: u64,
}

#[derive(Serialize)]
struct RawSample {
    schema: &'static str,
    schema_version: u32,
    fixture_revision: u32,
    runtime: RuntimeConfiguration,
    workload: WorkloadIdentity,
    exact_identity: ExactIdentity,
    sample_kind: &'static str,
    trial: u32,
    thermal_state: &'static str,
    phases: PhaseTimes,
    execution: ExecutionMeasurement,
    process_cpu_ns: Option<u64>,
    outcomes: OutcomeMeasurement,
    allocations: AllocationMeasurement,
    memory: MemoryMeasurement,
    latency: LatencyMeasurement,
    semantic_notes: [&'static str; 2],
}

#[derive(Clone)]
struct Observations {
    values: Rc<RefCell<Vec<u64>>>,
    starts: Rc<RefCell<Vec<Option<Instant>>>>,
    latencies: Rc<RefCell<Vec<u64>>>,
    accepted_values: Rc<Cell<u64>>,
    useful_outputs: Rc<Cell<u64>>,
    offered: Rc<Cell<u64>>,
    rejected: Rc<Cell<u64>>,
    sampled: Rc<Cell<u64>>,
    coalesced: Rc<Cell<u64>>,
    dropped: Rc<Cell<u64>>,
    retried: Rc<Cell<u64>>,
    recovery_started: Rc<RefCell<Option<Instant>>>,
    producer_stall_started: Rc<RefCell<Option<Instant>>>,
    producer_stall_ns: Rc<Cell<u64>>,
    pressure_cycles: Rc<Cell<u64>>,
    recovery_cycles: Rc<Cell<u64>>,
    terminal_requested: Rc<Cell<bool>>,
    observation_window_waiting: Rc<Cell<bool>>,
    stride: u64,
}

fn begin_producer_stall(observations: &Observations) {
    let mut started = observations.producer_stall_started.borrow_mut();
    if started.is_none() {
        *started = Some(Instant::now());
    }
}

fn finish_producer_stall(observations: &Observations) {
    if let Some(started) = observations.producer_stall_started.borrow_mut().take() {
        observations.producer_stall_ns.set(
            observations
                .producer_stall_ns
                .get()
                .saturating_add(started.elapsed().as_nanos() as u64),
        );
    }
}

enum BenchNode {
    Source {
        next: u64,
        end: u64,
        cord: usize,
        observations: Observations,
        retrying: bool,
        pressure: PressurePolicy,
        queue_capacity: u64,
        standing: bool,
        wake_mode: WakeMode,
        wake_armed: bool,
    },
    CoupledSource {
        next: u64,
        end: u64,
        branch_count: usize,
        observations: Observations,
        retrying: bool,
    },
    IsolatedDuplicator {
        input: usize,
        first_output: usize,
        branch_count: usize,
        retained_handle: Option<u64>,
        delivered: [bool; 32],
        cursor: usize,
        observations: Observations,
    },
    Map {
        input: usize,
        output: usize,
        observations: Observations,
    },
    Filter {
        input: usize,
        output: usize,
        observations: Observations,
    },
    Merge {
        inputs: [usize; 2],
        output: usize,
        cursor: usize,
    },
    Sink {
        input: usize,
        observations: Observations,
        slow_consumer_yields: u32,
        yields_remaining: u32,
        recovery_after_outputs: u64,
        completed: u64,
        records_recovery: bool,
        consumer_pattern: ConsumerPattern,
        consumer_burst_items: u64,
        burst_progress: u64,
    },
}

impl SchedulerNode for BenchNode {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn retained_value_usage(&self) -> RetainedValueUsage {
        match self {
            Self::IsolatedDuplicator {
                retained_handle: Some(_),
                ..
            } => RetainedValueUsage {
                values: 1,
                bytes: 8,
            },
            _ => RetainedValueUsage::default(),
        }
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match self {
            Self::Source {
                next,
                end,
                cord,
                observations,
                retrying,
                pressure,
                queue_capacity,
                standing,
                wake_mode,
                wake_armed,
            } => {
                if *next == *end {
                    if *standing {
                        if observations.terminal_requested.get() {
                            return SchedulerStep::Completed;
                        }
                        observations.observation_window_waiting.set(true);
                        io.wait_for_host_operation(Id("benchmark/observation-window"))
                            .unwrap();
                        return SchedulerStep::Pending;
                    }
                    return SchedulerStep::Completed;
                }
                if !matches!(*wake_mode, WakeMode::None) && !*retrying {
                    if !*wake_armed {
                        observations.observation_window_waiting.set(true);
                        match wake_mode {
                            WakeMode::HostOperation => io
                                .wait_for_host_operation(Id("benchmark/persistent-wake"))
                                .unwrap(),
                            WakeMode::Timer(advance_ticks) => io
                                .wait_for_timer(
                                    Id("benchmark/persistent-timer"),
                                    io.tick().saturating_add(*advance_ticks),
                                )
                                .unwrap(),
                            WakeMode::None => unreachable!(),
                        }
                        *wake_armed = true;
                        return SchedulerStep::Pending;
                    }
                    *wake_armed = false;
                    observations.observation_window_waiting.set(false);
                }
                let index = *next;
                if !*retrying {
                    observations.offered.set(observations.offered.get() + 1);
                }
                if !*retrying && index % observations.stride == 0 {
                    let sample = usize::try_from(index / observations.stride).unwrap();
                    observations.starts.borrow_mut()[sample] = Some(Instant::now());
                }
                let value = RuntimeValue {
                    handle: index,
                    accounted_bytes: 8,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                let outstanding = observations
                    .accepted_values
                    .get()
                    .saturating_sub(observations.coalesced.get())
                    .saturating_sub(observations.useful_outputs.get());
                let coalesce_target = matches!(pressure, PressurePolicy::Coalesce).then_some(0);
                match io.send(*cord, value, coalesce_target) {
                    Ok(SendStatus::Reserved) => {
                        finish_producer_stall(observations);
                        *next += 1;
                        *retrying = false;
                        observations
                            .accepted_values
                            .set(observations.accepted_values.get() + 1);
                        if matches!(pressure, PressurePolicy::Coalesce)
                            && outstanding >= *queue_capacity
                        {
                            observations.coalesced.set(observations.coalesced.get() + 1);
                        }
                        SchedulerStep::Progress
                    }
                    Ok(SendStatus::WouldBlock) => {
                        begin_producer_stall(observations);
                        observations.retried.set(observations.retried.get() + 1);
                        *retrying = true;
                        io.wait_for_output(*cord).unwrap();
                        SchedulerStep::Pending
                    }
                    Ok(SendStatus::Rejected) => {
                        finish_producer_stall(observations);
                        observations.rejected.set(observations.rejected.get() + 1);
                        *next += 1;
                        *retrying = false;
                        SchedulerStep::Progress
                    }
                    Ok(SendStatus::Dropped) => {
                        finish_producer_stall(observations);
                        let counter =
                            if matches!(pressure, PressurePolicy::Sample) && index % 2 != 0 {
                                &observations.sampled
                            } else {
                                &observations.dropped
                            };
                        counter.set(counter.get() + 1);
                        *next += 1;
                        *retrying = false;
                        SchedulerStep::Progress
                    }
                    Ok(SendStatus::Disconnected) => SchedulerStep::Completed,
                    Ok(SendStatus::Failed) => SchedulerStep::Failed {
                        code: Id("benchmark/pressure-failed"),
                    },
                    Ok(SendStatus::Terminated) => {
                        finish_producer_stall(observations);
                        SchedulerStep::Completed
                    }
                    Err(_) => SchedulerStep::Failed {
                        code: Id("benchmark/source-send-error"),
                    },
                }
            }
            Self::CoupledSource {
                next,
                end,
                branch_count,
                observations,
                retrying,
            } => {
                if *next == *end {
                    return SchedulerStep::Completed;
                }
                let index = *next;
                if !*retrying {
                    observations.offered.set(observations.offered.get() + 1);
                }
                if !*retrying && index % observations.stride == 0 {
                    let sample = usize::try_from(index / observations.stride).unwrap();
                    observations.starts.borrow_mut()[sample] = Some(Instant::now());
                }
                let value = RuntimeValue {
                    handle: index,
                    accounted_bytes: 8,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                let values = [value; 32];
                let targets = [None; 32];
                match io.send_coupled(0, &values[..*branch_count], &targets[..*branch_count]) {
                    Ok(SendStatus::Reserved) => {
                        finish_producer_stall(observations);
                        *next += 1;
                        *retrying = false;
                        observations
                            .accepted_values
                            .set(observations.accepted_values.get() + 1);
                        SchedulerStep::Progress
                    }
                    Ok(SendStatus::WouldBlock) => {
                        begin_producer_stall(observations);
                        observations.retried.set(observations.retried.get() + 1);
                        *retrying = true;
                        for cord in 0..*branch_count {
                            io.wait_for_output(cord).unwrap();
                        }
                        SchedulerStep::Pending
                    }
                    Ok(SendStatus::Terminated) => SchedulerStep::Completed,
                    Ok(
                        SendStatus::Rejected
                        | SendStatus::Dropped
                        | SendStatus::Disconnected
                        | SendStatus::Failed,
                    ) => SchedulerStep::Failed {
                        code: Id("benchmark/coupled-fanout-publication-failed"),
                    },
                    Err(_) => SchedulerStep::Failed {
                        code: Id("benchmark/coupled-fanout-send-error"),
                    },
                }
            }
            Self::IsolatedDuplicator {
                input,
                first_output,
                branch_count,
                retained_handle,
                delivered,
                cursor,
                observations,
            } => {
                if retained_handle.is_none() {
                    match io.receive(*input) {
                        Ok(Some(value)) => {
                            assert_eq!(value.accounted_bytes, 8);
                            assert_eq!(value.envelope, RuntimeValueEnvelope::EMPTY);
                            *retained_handle = Some(value.handle);
                            delivered[..*branch_count].fill(false);
                            *cursor = usize::try_from(value.handle).unwrap() % *branch_count;
                            return SchedulerStep::Progress;
                        }
                        _ if matches!(io.input_state(*input), Ok(FlowQueueState::Completed)) => {
                            return SchedulerStep::Completed;
                        }
                        _ => {
                            io.wait_for_input(*input).unwrap();
                            return SchedulerStep::Pending;
                        }
                    }
                }

                let value = RuntimeValue {
                    handle: retained_handle.expect("isolated duplicator retains one input"),
                    accounted_bytes: 8,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                for offset in 0..*branch_count {
                    let branch = (*cursor + offset) % *branch_count;
                    if delivered[branch] {
                        continue;
                    }
                    let cord = *first_output + branch;
                    match io.send(cord, value, None) {
                        Ok(SendStatus::Reserved) => {
                            delivered[branch] = true;
                            *cursor = (branch + 1) % *branch_count;
                            if delivered[..*branch_count].iter().all(|value| *value) {
                                *retained_handle = None;
                            }
                            return SchedulerStep::Progress;
                        }
                        Ok(SendStatus::WouldBlock) => {
                            observations.retried.set(observations.retried.get() + 1);
                            io.wait_for_output(cord).unwrap();
                        }
                        Ok(SendStatus::Terminated) => return SchedulerStep::Completed,
                        Ok(
                            SendStatus::Rejected
                            | SendStatus::Dropped
                            | SendStatus::Disconnected
                            | SendStatus::Failed,
                        ) => {
                            return SchedulerStep::Failed {
                                code: Id("benchmark/isolated-fanout-publication-failed"),
                            };
                        }
                        Err(_) => {
                            return SchedulerStep::Failed {
                                code: Id("benchmark/isolated-fanout-send-error"),
                            };
                        }
                    }
                }
                SchedulerStep::Pending
            }
            Self::Map {
                input,
                output,
                observations,
            } => match io.receive(*input) {
                Ok(Some(value)) => {
                    let index = usize::try_from(value.handle).unwrap();
                    let mapped = observations.values.borrow()[index].wrapping_add(2);
                    match io.send(*output, value, None) {
                        Ok(SendStatus::Reserved) => {
                            observations.values.borrow_mut()[index] = black_box(mapped);
                            SchedulerStep::Progress
                        }
                        Ok(SendStatus::WouldBlock) => {
                            io.wait_for_output(*output).unwrap();
                            SchedulerStep::Pending
                        }
                        _ => SchedulerStep::Failed {
                            code: Id("benchmark/map-output-rejected"),
                        },
                    }
                }
                _ if matches!(io.input_state(*input), Ok(FlowQueueState::Completed)) => {
                    SchedulerStep::Completed
                }
                _ => {
                    io.wait_for_input(*input).unwrap();
                    SchedulerStep::Pending
                }
            },
            Self::Filter {
                input,
                output,
                observations,
            } => match io.receive(*input) {
                Ok(Some(value)) => {
                    let index = usize::try_from(value.handle).unwrap();
                    if observations.values.borrow()[index] % 2 == 0 {
                        match io.send(*output, value, None) {
                            Ok(SendStatus::Reserved) => SchedulerStep::Progress,
                            Ok(SendStatus::WouldBlock) => {
                                io.wait_for_output(*output).unwrap();
                                SchedulerStep::Pending
                            }
                            _ => SchedulerStep::Failed {
                                code: Id("benchmark/filter-output-rejected"),
                            },
                        }
                    } else {
                        SchedulerStep::Progress
                    }
                }
                _ if matches!(io.input_state(*input), Ok(FlowQueueState::Completed)) => {
                    SchedulerStep::Completed
                }
                _ => {
                    io.wait_for_input(*input).unwrap();
                    SchedulerStep::Pending
                }
            },
            Self::Merge {
                inputs,
                output,
                cursor,
            } => {
                for offset in 0..2 {
                    let branch = (*cursor + offset) % 2;
                    if let Ok(Some(value)) = io.receive(inputs[branch]) {
                        return match io.send(*output, value, None) {
                            Ok(SendStatus::Reserved) => {
                                *cursor = (branch + 1) % 2;
                                SchedulerStep::Progress
                            }
                            Ok(SendStatus::WouldBlock) => {
                                io.wait_for_output(*output).unwrap();
                                SchedulerStep::Pending
                            }
                            _ => SchedulerStep::Failed {
                                code: Id("benchmark/merge-output-rejected"),
                            },
                        };
                    }
                }
                if inputs
                    .iter()
                    .all(|input| matches!(io.input_state(*input), Ok(FlowQueueState::Completed)))
                {
                    SchedulerStep::Completed
                } else {
                    for input in inputs {
                        io.wait_for_input(*input).unwrap();
                    }
                    SchedulerStep::Pending
                }
            }
            Self::Sink {
                input,
                observations,
                slow_consumer_yields,
                yields_remaining,
                recovery_after_outputs,
                completed,
                records_recovery,
                consumer_pattern,
                consumer_burst_items,
                burst_progress,
            } => {
                let pause_active = match consumer_pattern {
                    ConsumerPattern::Sustained => *completed < *recovery_after_outputs,
                    ConsumerPattern::Bursty => *burst_progress == *consumer_burst_items,
                };
                if pause_active
                    && matches!(consumer_pattern, ConsumerPattern::Bursty)
                    && matches!(
                        io.input_state(*input),
                        Ok(FlowQueueState::Completed
                            | FlowQueueState::Cancelled
                            | FlowQueueState::Failed
                            | FlowQueueState::Disconnected)
                    )
                {
                    return SchedulerStep::Completed;
                }
                if pause_active && *yields_remaining > 0 {
                    *yields_remaining -= 1;
                    io.consume_work(LIMITS.max_step_work).unwrap();
                    if *yields_remaining == 0 && matches!(consumer_pattern, ConsumerPattern::Bursty)
                    {
                        *burst_progress = 0;
                        if *records_recovery {
                            observations
                                .recovery_cycles
                                .set(observations.recovery_cycles.get() + 1);
                        }
                    }
                    return SchedulerStep::Yielded;
                }
                match io.receive(*input) {
                    Ok(Some(value)) => {
                        *completed += 1;
                        if matches!(consumer_pattern, ConsumerPattern::Bursty) {
                            *burst_progress += 1;
                            if *burst_progress == *consumer_burst_items {
                                *yields_remaining = *slow_consumer_yields;
                                if *records_recovery {
                                    observations
                                        .pressure_cycles
                                        .set(observations.pressure_cycles.get() + 1);
                                }
                            }
                        }
                        observations
                            .useful_outputs
                            .set(observations.useful_outputs.get() + 1);
                        if *records_recovery
                            && *completed == *recovery_after_outputs
                            && observations.recovery_started.borrow().is_none()
                        {
                            *observations.recovery_started.borrow_mut() = Some(Instant::now());
                        }
                        if value.handle % observations.stride == 0 {
                            let sample =
                                usize::try_from(value.handle / observations.stride).unwrap();
                            if let Some(start) = observations.starts.borrow()[sample] {
                                observations
                                    .latencies
                                    .borrow_mut()
                                    .push(start.elapsed().as_nanos() as u64);
                            }
                        }
                        if matches!(consumer_pattern, ConsumerPattern::Sustained) {
                            *yields_remaining = *slow_consumer_yields;
                        }
                        SchedulerStep::Progress
                    }
                    _ if matches!(
                        io.input_state(*input),
                        Ok(FlowQueueState::Completed
                            | FlowQueueState::Cancelled
                            | FlowQueueState::Failed
                            | FlowQueueState::Disconnected)
                    ) =>
                    {
                        SchedulerStep::Completed
                    }
                    _ => {
                        io.wait_for_input(*input).unwrap();
                        SchedulerStep::Pending
                    }
                }
            }
        }
    }
}

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn semantic_digest(parts: &[&str]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.comparative-benchmark\0");
    for part in parts {
        digest.update(u64::try_from(part.len()).unwrap().to_le_bytes());
        digest.update(part.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn recovery_after_outputs(args: &Args) -> u64 {
    if matches!(
        args.workload,
        Workload::PersistentWake | Workload::PersistentTimer
    ) {
        return 0;
    }
    if matches!(args.consumer_pattern, ConsumerPattern::Bursty) {
        return 0;
    }
    if args.termination_request.stop_policy().is_some() {
        return args.values;
    }
    if matches!(args.workload, Workload::Overload | Workload::Fanout) {
        (u64::from(args.queue_items) * 2)
            .min(args.values / 2)
            .max(1)
    } else {
        0
    }
}

fn current_binary_digest() -> ArtifactDigest {
    static DIGEST: OnceLock<ArtifactDigest> = OnceLock::new();
    *DIGEST.get_or_init(|| {
        let executable = std::env::current_exe().expect("benchmark executable path is available");
        let bytes = std::fs::read(executable).expect("benchmark executable is readable");
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        ArtifactDigest::from_bytes(digest)
    })
}

fn leaked(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn pin(id: &'static str, kind: &'static str) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: semantic_digest(&[kind, id]),
    }
}

fn profile(args: &Args) -> ExecutionProfile<'static> {
    let mut limits = LIMITS;
    if args.termination_request.stop_policy().is_some() {
        limits.cancellation_ticks = u64::from(args.queue_items)
            .saturating_mul(u64::from(args.slow_consumer_yields) + 2)
            .saturating_add(8);
    }
    if matches!(args.workload, Workload::Fanout) {
        if matches!(args.fanout_mode, FanoutPublication::Coupled) {
            limits.max_output_reservations = args.fanout_branches;
            limits.max_output_bytes = u64::from(args.fanout_branches) * 8;
        } else {
            limits.max_retained_values = 1;
            limits.max_retained_bytes = 8;
            limits.implementation_memory_bytes += ISOLATED_CLAIMS[1].bytes;
        }
    }
    let mut value = ExecutionProfile {
        id: Id("benchmark/reference-profile"),
        schema_version: 0,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits,
        representations: &[],
        memory_claims: if matches!(args.workload, Workload::Fanout)
            && matches!(args.fanout_mode, FanoutPublication::Isolated)
        {
            &ISOLATED_CLAIMS
        } else {
            &CLAIMS
        },
        checkpoint: None,
    };
    value.semantic_hash = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

fn machine(
    profile: &'static ExecutionProfile<'static>,
    node: &ResolvedPlanNode<'static>,
) -> ImplementationMachine {
    ImplementationMachine::instantiate(
        profile,
        InstantiationContext {
            instance: node.instance,
            implementation: node.implementation,
            artifact: node.artifact,
            execution_profile_hash: profile.semantic_hash,
            configuration_validated: true,
            caller_memory_bytes: profile.limits.implementation_memory_bytes,
            required_resource_bindings: &[],
            provided_resource_bindings: &[],
            required_grants: &[],
            provided_grants: &[],
            cancellation_scope: Id("run"),
        },
    )
    .unwrap()
}

struct PreparedRun {
    plan: &'static ExecutionPlan<'static>,
    drivers: Vec<BenchNode>,
    observations: Observations,
    assembly_ns: u64,
    seal_ns: u64,
}

fn prepare(args: &Args) -> PreparedRun {
    assert!(args.operators > 0, "operators must be positive");
    assert!(args.values > 0, "values must be positive");
    assert!(args.queue_items > 0, "queue capacity must be positive");
    assert!(
        args.latency_sample_stride > 0,
        "sample stride must be positive"
    );
    assert!(
        !matches!(args.workload, Workload::BoundedAsync),
        "the single-lane reference executor cannot claim an asynchronous boundary"
    );
    if args.termination_request.stop_policy().is_some() {
        assert!(
            matches!(
                args.workload,
                Workload::Overload | Workload::PersistentWake | Workload::PersistentTimer
            ),
            "requested termination requires an exact persistent or pressured fixture"
        );
        if matches!(args.workload, Workload::Overload) {
            assert!(
                args.cancel_after_offers > u64::from(args.queue_items),
                "cancellation must occur after pressure begins"
            );
        }
        match args.session_mode {
            SessionMode::Finite => {
                assert!(
                    matches!(args.pressure_policy, PressurePolicy::Block),
                    "finite cancellation is measured under FIFO block pressure"
                );
                assert!(
                    args.cancel_after_offers < args.values,
                    "finite cancellation must occur before source completion"
                );
            }
            SessionMode::Persistent => {
                if matches!(args.workload, Workload::Overload) {
                    assert!(
                        matches!(
                            args.pressure_policy,
                            PressurePolicy::Block
                                | PressurePolicy::Reject
                                | PressurePolicy::Coalesce
                                | PressurePolicy::Sample
                                | PressurePolicy::DropDisposable
                        ),
                        "persistent cancellation requires a nonterminal pressure policy"
                    );
                } else {
                    assert!(
                        matches!(args.termination_request, TerminationRequest::Drain),
                        "persistent wake residency ends with an exact Drain request"
                    );
                }
                assert_eq!(
                    args.cancel_after_offers, args.values,
                    "persistent cancellation begins at the exact observation offer boundary"
                );
            }
        }
    } else {
        assert_eq!(
            args.cancel_after_offers, 0,
            "complete fixtures do not carry an unused cancellation threshold"
        );
        assert!(
            matches!(args.session_mode, SessionMode::Finite),
            "persistent sessions require an explicit terminal request"
        );
    }
    if matches!(args.session_mode, SessionMode::Persistent) {
        assert!(
            matches!(
                args.workload,
                Workload::Overload | Workload::PersistentWake | Workload::PersistentTimer
            ),
            "persistent sessions require an exact persistent workload"
        );
        assert!(
            args.session_pump_quantum > 0,
            "persistent sessions require a bounded positive host pump quantum"
        );
    } else {
        assert_eq!(
            args.session_pump_quantum, 0,
            "finite executor fixtures do not carry an unused session pump quantum"
        );
    }
    if matches!(
        args.workload,
        Workload::PersistentWake | Workload::PersistentTimer
    ) {
        assert!(
            matches!(args.session_mode, SessionMode::Persistent),
            "persistent residency requires production ExactRunSession ownership"
        );
        assert_eq!(
            args.operators, 1,
            "persistent residency uses one source/sink boundary"
        );
        assert!(
            matches!(args.pressure_policy, PressurePolicy::Block),
            "persistent residency uses the fixed FIFO block policy"
        );
        assert_eq!(
            args.slow_consumer_yields, 0,
            "persistent residency measures standing operation without a slow consumer"
        );
        assert!(
            args.residency_plateau_after_wakes > 0
                && args.residency_plateau_after_wakes < args.values,
            "the residency checkpoint must follow warm wake cycles and precede the final wake"
        );
    } else {
        assert_eq!(
            args.residency_plateau_after_wakes, 0,
            "other fixtures do not carry an unused residency checkpoint"
        );
    }
    if matches!(args.workload, Workload::PersistentTimer) {
        assert!(
            args.timer_advance_ticks > args.session_pump_quantum,
            "persistent timer deadlines must remain ahead of one bounded host pump"
        );
    } else {
        assert_eq!(
            args.timer_advance_ticks, 0,
            "non-timer fixtures do not carry an unused timer advance"
        );
    }
    if matches!(args.consumer_pattern, ConsumerPattern::Bursty) {
        assert!(
            matches!(args.workload, Workload::Overload | Workload::Fanout),
            "bursty consumers require an overload or fan-out fixture"
        );
        assert!(
            args.termination_request.stop_policy().is_none(),
            "bursty and requested-cancellation identities are measured separately"
        );
        assert!(
            args.consumer_burst_items > 0
                && args.consumer_burst_items.saturating_mul(2) < args.values,
            "bursty consumers require at least two complete bounded bursts"
        );
    } else {
        assert_eq!(
            args.consumer_burst_items, 0,
            "sustained consumers do not carry an unused burst size"
        );
    }
    if matches!(args.workload, Workload::Overload) {
        assert_eq!(args.operators, 1, "overload uses one source/sink boundary");
        assert!(
            args.slow_consumer_yields > 0,
            "overload requires a slow consumer region"
        );
    } else if matches!(args.workload, Workload::Fanout) {
        assert_eq!(args.operators, 1, "fan-out uses one publication boundary");
        assert!(
            matches!(args.fanout_branches, 2 | 8 | 32),
            "fan-out branches must be 2, 8, or 32"
        );
        assert!(
            matches!(args.pressure_policy, PressurePolicy::Block),
            "the current fan-out slice uses FIFO block pressure"
        );
        assert!(
            args.slow_consumer_yields > 0,
            "fan-out requires at least one slow branch"
        );
    } else {
        assert!(
            matches!(args.pressure_policy, PressurePolicy::Block),
            "local-depth workloads use the fixed FIFO block policy"
        );
    }

    let assembly_started = Instant::now();
    let profile = Box::leak(Box::new(profile(args)));
    let observation = Box::leak(Box::new([PlanHostObservation {
        id: Id("benchmark/host-observation"),
        host: Id("host/local"),
        boot_id: Id("benchmark/process"),
        semantic_hash: hash(1),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: u64::MAX,
    }]));
    let value_count = usize::try_from(args.values).expect("value count fits usize");
    let sample_count = usize::try_from(args.values.div_ceil(args.latency_sample_stride))
        .expect("sample count fits usize");
    let latency_capacity =
        sample_count.saturating_mul(if matches!(args.workload, Workload::Fanout) {
            usize::from(args.fanout_branches)
        } else {
            1
        });
    let observations = Observations {
        values: Rc::new(RefCell::new((0..args.values).collect())),
        starts: Rc::new(RefCell::new(vec![None; sample_count])),
        latencies: Rc::new(RefCell::new(Vec::with_capacity(latency_capacity))),
        accepted_values: Rc::new(Cell::new(0)),
        useful_outputs: Rc::new(Cell::new(0)),
        offered: Rc::new(Cell::new(0)),
        rejected: Rc::new(Cell::new(0)),
        sampled: Rc::new(Cell::new(0)),
        coalesced: Rc::new(Cell::new(0)),
        dropped: Rc::new(Cell::new(0)),
        retried: Rc::new(Cell::new(0)),
        recovery_started: Rc::new(RefCell::new(None)),
        producer_stall_started: Rc::new(RefCell::new(None)),
        producer_stall_ns: Rc::new(Cell::new(0)),
        pressure_cycles: Rc::new(Cell::new(0)),
        recovery_cycles: Rc::new(Cell::new(0)),
        terminal_requested: Rc::new(Cell::new(false)),
        observation_window_waiting: Rc::new(Cell::new(false)),
        stride: args.latency_sample_stride,
    };
    assert_eq!(observations.values.borrow().len(), value_count);

    let transform_count = match args.workload {
        Workload::Merge => args.operators.saturating_sub(1),
        Workload::Overload => 0,
        Workload::Fanout => 0,
        Workload::PersistentWake => 0,
        Workload::PersistentTimer => 0,
        _ => args.operators,
    };
    let mut node_roles = if matches!(args.workload, Workload::Merge) {
        vec!["source", "source", "merge"]
    } else if matches!(args.workload, Workload::Fanout) {
        let mut roles = vec!["source"];
        if matches!(args.fanout_mode, FanoutPublication::Isolated) {
            roles.push("duplicator");
        }
        let non_sink = roles.len();
        roles.resize(non_sink + usize::from(args.fanout_branches), "sink");
        roles
    } else {
        vec!["source"]
    };
    for transform in 0..transform_count {
        node_roles.push(
            if matches!(args.workload, Workload::MapFilter) && transform % 2 == 1 {
                "filter"
            } else {
                "map"
            },
        );
    }
    if !matches!(args.workload, Workload::Fanout) {
        node_roles.push("sink");
    }
    let node_count = node_roles.len();
    let cord_count = node_count - 1;
    let artifact_id = Id("benchmark/conduit-benchmark-binary");
    let artifacts = vec![PlanArtifact {
        id: artifact_id,
        digest: current_binary_digest(),
    }];
    let mut nodes = Vec::with_capacity(node_count);
    for (index, role) in node_roles.iter().enumerate() {
        nodes.push(ResolvedPlanNode {
            instance: InstancePath::new(leaked(format!("root/node-{index}"))).unwrap(),
            contract: pin(leaked(format!("benchmark/{role}-contract")), "contract"),
            implementation: pin(
                leaked(format!("benchmark/{role}-implementation")),
                "implementation",
            ),
            lifecycle_policy: pin("benchmark/lifecycle", "lifecycle"),
            execution_profile: Some(profile),
            artifact: artifact_id,
            host_observation: observation[0].id,
            host: observation[0].host,
            allocation: PlanResourceBudget {
                memory_bytes: 2_048,
                cpu_units: 1,
                ..PlanResourceBudget::ZERO
            },
            required_resources: &[],
            required_effects: &[],
        });
    }

    let capacity =
        FlowCapacity::new(args.queue_items, 16, u64::from(args.queue_items) * 16).unwrap();
    let pressure = if matches!(args.workload, Workload::Overload) {
        match args.pressure_policy {
            PressurePolicy::Block => Pressure::Block(BlockingFairness::Fifo),
            PressurePolicy::Reject => Pressure::Reject,
            PressurePolicy::Coalesce => Pressure::Coalesce {
                relation: Id("benchmark/latest-wins"),
            },
            PressurePolicy::Sample => Pressure::Sample(SampleSchedule::new(2, 0).unwrap()),
            PressurePolicy::DropDisposable => Pressure::DropDisposable,
            PressurePolicy::Disconnect => Pressure::Disconnect,
            PressurePolicy::Fail => Pressure::Fail,
        }
    } else {
        Pressure::Block(BlockingFairness::Fifo)
    };
    let flow = FlowPolicy::new(
        capacity,
        pressure,
        FlowWatermarks::new(0, args.queue_items, capacity).unwrap(),
    )
    .unwrap();
    let coalescers = [Id("benchmark/latest-wins")];
    let type_facts = FlowTypeFacts {
        disposable: TraitProof::Proven,
        coalescers: Some(&coalescers),
    };
    assert_eq!(
        flow.assess_type_facts(type_facts).outcome,
        CompatibilityOutcome::Compatible,
        "the exact benchmark value type must prove every selected loss policy"
    );
    let port = |node: &ResolvedPlanNode<'static>, direction, name| ResolvedPlanPort {
        node: node.instance,
        port: Id(name),
        direction,
        port_contract_hash: hash(if matches!(direction, Direction::Input) {
            5
        } else {
            6
        }),
        value_type: VALUE_TYPE,
    };
    let mut cords = Vec::with_capacity(cord_count);
    let mut drivers = Vec::with_capacity(node_count);
    let mut fanouts = Vec::new();

    if matches!(args.workload, Workload::Merge) {
        cords.push(ResolvedPlanCord {
            id: Id("benchmark/cord-0"),
            from: port(&nodes[0], Direction::Output, "out"),
            to: port(&nodes[2], Direction::Input, "left"),
            flow,
            queue_memory_bytes: u64::from(args.queue_items) * 16,
        });
        cords.push(ResolvedPlanCord {
            id: Id("benchmark/cord-1"),
            from: port(&nodes[1], Direction::Output, "out"),
            to: port(&nodes[2], Direction::Input, "right"),
            flow,
            queue_memory_bytes: u64::from(args.queue_items) * 16,
        });
        let split = args.values / 2;
        drivers.push(BenchNode::Source {
            next: 0,
            end: split,
            cord: 0,
            observations: observations.clone(),
            retrying: false,
            pressure: args.pressure_policy,
            queue_capacity: u64::from(args.queue_items),
            standing: false,
            wake_mode: WakeMode::None,
            wake_armed: false,
        });
        drivers.push(BenchNode::Source {
            next: split,
            end: args.values,
            cord: 1,
            observations: observations.clone(),
            retrying: false,
            pressure: args.pressure_policy,
            queue_capacity: u64::from(args.queue_items),
            standing: false,
            wake_mode: WakeMode::None,
            wake_armed: false,
        });
        drivers.push(BenchNode::Merge {
            inputs: [0, 1],
            output: 2,
            cursor: 0,
        });
        let mut previous_node = 2;
        let mut previous_cord = 2;
        for transform in 0..transform_count {
            let node = 3 + transform;
            cords.push(ResolvedPlanCord {
                id: Id(leaked(format!("benchmark/cord-{previous_cord}"))),
                from: port(&nodes[previous_node], Direction::Output, "out"),
                to: port(&nodes[node], Direction::Input, "in"),
                flow,
                queue_memory_bytes: u64::from(args.queue_items) * 16,
            });
            drivers.push(BenchNode::Map {
                input: previous_cord,
                output: previous_cord + 1,
                observations: observations.clone(),
            });
            previous_node = node;
            previous_cord += 1;
        }
        let sink_node = node_count - 1;
        cords.push(ResolvedPlanCord {
            id: Id(leaked(format!("benchmark/cord-{previous_cord}"))),
            from: port(&nodes[previous_node], Direction::Output, "out"),
            to: port(&nodes[sink_node], Direction::Input, "in"),
            flow,
            queue_memory_bytes: u64::from(args.queue_items) * 16,
        });
        drivers.push(BenchNode::Sink {
            input: previous_cord,
            observations: observations.clone(),
            slow_consumer_yields: 0,
            yields_remaining: 0,
            recovery_after_outputs: 0,
            completed: 0,
            records_recovery: false,
            consumer_pattern: ConsumerPattern::Sustained,
            consumer_burst_items: 0,
            burst_progress: 0,
        });
    } else if matches!(args.workload, Workload::Fanout) {
        let branch_count = usize::from(args.fanout_branches);
        let isolated = matches!(args.fanout_mode, FanoutPublication::Isolated);
        if isolated {
            cords.push(ResolvedPlanCord {
                id: Id("benchmark/duplicator-input"),
                from: port(&nodes[0], Direction::Output, "out"),
                to: port(&nodes[1], Direction::Input, "in"),
                flow,
                queue_memory_bytes: u64::from(args.queue_items) * 16,
            });
            drivers.push(BenchNode::Source {
                next: 0,
                end: args.values,
                cord: 0,
                observations: observations.clone(),
                retrying: false,
                pressure: PressurePolicy::Block,
                queue_capacity: u64::from(args.queue_items),
                standing: false,
                wake_mode: WakeMode::None,
                wake_armed: false,
            });
            drivers.push(BenchNode::IsolatedDuplicator {
                input: 0,
                first_output: 1,
                branch_count,
                retained_handle: None,
                delivered: [false; 32],
                cursor: 0,
                observations: observations.clone(),
            });
        } else {
            drivers.push(BenchNode::CoupledSource {
                next: 0,
                end: args.values,
                branch_count,
                observations: observations.clone(),
                retrying: false,
            });
        }
        for branch in 0..branch_count {
            let sink_node = branch + if isolated { 2 } else { 1 };
            let cord_index = branch + usize::from(isolated);
            cords.push(ResolvedPlanCord {
                id: Id(leaked(format!("benchmark/branch-{branch}"))),
                from: port(&nodes[usize::from(isolated)], Direction::Output, "out"),
                to: port(&nodes[sink_node], Direction::Input, "in"),
                flow,
                queue_memory_bytes: u64::from(args.queue_items) * 16,
            });
            let slow = matches!(args.slow_branches, SlowBranches::All) || branch == 0;
            drivers.push(BenchNode::Sink {
                input: cord_index,
                observations: observations.clone(),
                slow_consumer_yields: if slow { args.slow_consumer_yields } else { 0 },
                yields_remaining: if slow { args.slow_consumer_yields } else { 0 },
                recovery_after_outputs: recovery_after_outputs(args),
                completed: 0,
                records_recovery: slow && branch == 0,
                consumer_pattern: if slow {
                    args.consumer_pattern
                } else {
                    ConsumerPattern::Sustained
                },
                consumer_burst_items: if slow { args.consumer_burst_items } else { 0 },
                burst_progress: if slow && matches!(args.consumer_pattern, ConsumerPattern::Bursty)
                {
                    args.consumer_burst_items
                } else {
                    0
                },
            });
        }
        let branches = Box::leak(
            cords
                .iter()
                .skip(usize::from(isolated))
                .map(|cord| cord.id)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        fanouts.push(PlanFanOut {
            id: Id(if isolated {
                "benchmark/isolated-fanout"
            } else {
                "benchmark/coupled-fanout"
            }),
            producer: port(&nodes[usize::from(isolated)], Direction::Output, "out"),
            mode: if isolated {
                FanOutMode::Isolated
            } else {
                FanOutMode::Coupled
            },
            branches,
            duplicator: isolated.then_some(nodes[1].instance),
            duplicator_input: isolated.then_some(cords[0].id),
            duplication: DuplicationRule::Copy(pin("benchmark/u64-copy", "duplication-rule")),
        });
    } else {
        drivers.push(BenchNode::Source {
            next: 0,
            end: args.values,
            cord: 0,
            observations: observations.clone(),
            retrying: false,
            pressure: args.pressure_policy,
            queue_capacity: u64::from(args.queue_items),
            standing: matches!(args.session_mode, SessionMode::Persistent),
            wake_mode: match args.workload {
                Workload::PersistentWake => WakeMode::HostOperation,
                Workload::PersistentTimer => WakeMode::Timer(args.timer_advance_ticks),
                _ => WakeMode::None,
            },
            wake_armed: false,
        });
        for transform in 0..transform_count {
            let from = transform;
            let to = transform + 1;
            cords.push(ResolvedPlanCord {
                id: Id(leaked(format!("benchmark/cord-{transform}"))),
                from: port(&nodes[from], Direction::Output, "out"),
                to: port(&nodes[to], Direction::Input, "in"),
                flow,
                queue_memory_bytes: u64::from(args.queue_items) * 16,
            });
            let filter = matches!(args.workload, Workload::MapFilter) && transform % 2 == 1;
            drivers.push(if filter {
                BenchNode::Filter {
                    input: transform,
                    output: transform + 1,
                    observations: observations.clone(),
                }
            } else {
                BenchNode::Map {
                    input: transform,
                    output: transform + 1,
                    observations: observations.clone(),
                }
            });
        }
        let sink_node = node_count - 1;
        cords.push(ResolvedPlanCord {
            id: Id(leaked(format!("benchmark/cord-{transform_count}"))),
            from: port(&nodes[transform_count], Direction::Output, "out"),
            to: port(&nodes[sink_node], Direction::Input, "in"),
            flow,
            queue_memory_bytes: u64::from(args.queue_items) * 16,
        });
        drivers.push(BenchNode::Sink {
            input: transform_count,
            observations: observations.clone(),
            slow_consumer_yields: if matches!(args.workload, Workload::Overload) {
                args.slow_consumer_yields
            } else {
                0
            },
            yields_remaining: if matches!(args.workload, Workload::Overload) {
                args.slow_consumer_yields
            } else {
                0
            },
            recovery_after_outputs: recovery_after_outputs(args),
            completed: 0,
            records_recovery: matches!(args.workload, Workload::Overload)
                && args.termination_request.stop_policy().is_none(),
            consumer_pattern: args.consumer_pattern,
            consumer_burst_items: args.consumer_burst_items,
            burst_progress: if matches!(args.consumer_pattern, ConsumerPattern::Bursty) {
                args.consumer_burst_items
            } else {
                0
            },
        });
    }

    let assembly_ns = assembly_started.elapsed().as_nanos() as u64;
    let artifacts = Box::leak(artifacts.into_boxed_slice());
    let nodes = Box::leak(nodes.into_boxed_slice());
    let cords = Box::leak(cords.into_boxed_slice());
    let fanouts = Box::leak(fanouts.into_boxed_slice());
    let mut plan = ExecutionPlan {
        schema_version: 0,
        identity: ZERO,
        source_semantic_hash: semantic_digest(&[
            "logical-source",
            match args.workload {
                Workload::Map => "map",
                Workload::MapFilter => "map-filter",
                Workload::Merge => "merge",
                Workload::BoundedAsync => "bounded-async",
                Workload::Overload => "overload",
                Workload::Fanout => "fanout",
                Workload::SharedPayloadFanout => "shared-payload-fanout",
                Workload::PersistentWake => "persistent-wake",
                Workload::PersistentTimer => "persistent-timer",
            },
            &args.operators.to_string(),
            &args.values.to_string(),
            &args.queue_items.to_string(),
            args.pressure_policy.as_str(),
            &args.slow_consumer_yields.to_string(),
            &args.fanout_branches.to_string(),
            args.slow_branches.as_str(),
            args.consumer_pattern.as_str(),
            &args.consumer_burst_items.to_string(),
            args.session_mode.as_str(),
            &args.session_pump_quantum.to_string(),
            &args.residency_plateau_after_wakes.to_string(),
            &args.timer_advance_ticks.to_string(),
        ]),
        resolver: pin("benchmark/resolver", "resolver"),
        resolver_policy_hash: semantic_digest(&["resolver-policy", "exact-local-reference"]),
        created_at: AuthorityTime {
            basis: Id("clock/monotonic"),
            tick: 1,
        },
        budget: PlanResourceBudget {
            memory_bytes: 512 * 1024 * 1024,
            storage_bytes: 0,
            cpu_units: u32::try_from(node_count).unwrap(),
            timers: 0,
            transports: 0,
            checkpoints: 0,
            evidence_bytes: 256 * 1024 * 1024,
        },
        host_observations: observation,
        resources: &[],
        workloads: &[],
        artifacts,
        nodes,
        cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &[],
        distributed_cords: &[],
        fanouts,
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
    let seal_started = Instant::now();
    let scratch_count = plan.validation_scratch_count().unwrap().max(1);
    plan.identity = plan.semantic_hash(&mut vec![ZERO; scratch_count]).unwrap();
    validate_execution_plan(
        &plan,
        PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        },
        &mut vec![ZERO; scratch_count],
    )
    .unwrap_or_else(|error| panic!("benchmark plan validation failed: {error}"));
    let seal_ns = seal_started.elapsed().as_nanos() as u64;
    let plan = Box::leak(Box::new(plan));
    PreparedRun {
        plan,
        drivers,
        observations,
        assembly_ns,
        seal_ns,
    }
}

fn process_cpu_ns() -> Option<u64> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `value` is a valid writable timespec and the clock id has no extra contract.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut value) };
    (status == 0).then(|| value.tv_sec as u64 * 1_000_000_000 + value.tv_nsec as u64)
}

fn proc_status_bytes(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    let kib = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    kib.checked_mul(1024)
}

fn shared_payload_source(
    branches: u16,
    payload_bytes: usize,
    binding: PayloadBinding,
) -> (String, String) {
    assert!(matches!(branches, 1 | 2 | 8 | 32));
    assert!(matches!(payload_bytes, 1024 | 1_048_576));
    let payload = (0..payload_bytes)
        .map(|index| char::from(b'a' + u8::try_from(index % 26).unwrap()))
        .collect::<String>();
    let mut source = String::from("panel 0\n\nsource: std/literal {\n    value = \"");
    source.push_str(&payload);
    source.push_str("\"\n}\n");
    for sink in 0..branches {
        if matches!(binding, PayloadBinding::BranchLocalUppercaseCopy) {
            writeln!(source, "copy_{sink}: text/uppercase").unwrap();
        }
        writeln!(source, "sink_{sink}: display/text").unwrap();
    }
    let cord = |source: &mut String, from: &str, to: &str| {
        writeln!(
            source,
            "{from} > {to} {{ capacity = 1 max_value_bytes = {payload_bytes} max_queued_bytes = {payload_bytes} low_watermark = 0 high_watermark = 1 pressure = block }}"
        )
        .unwrap();
    };
    for sink in 0..branches {
        if matches!(binding, PayloadBinding::BranchLocalUppercaseCopy) {
            cord(&mut source, "source.value", &format!("copy_{sink}.text"));
            cord(
                &mut source,
                &format!("copy_{sink}.text"),
                &format!("sink_{sink}.text"),
            );
        } else {
            cord(&mut source, "source.value", &format!("sink_{sink}.text"));
        }
    }
    (source, payload)
}

fn run_shared_payload_sample(
    args: &Args,
    sample_kind: &'static str,
    trial: u32,
    thermal_state: &'static str,
) -> RawSample {
    assert!(matches!(args.workload, Workload::SharedPayloadFanout));
    assert_eq!(
        args.values, 1,
        "shared payload trials publish one exact value"
    );
    assert_eq!(
        args.operators, 1,
        "shared payload fan-out is one logical publication boundary"
    );
    assert_eq!(
        args.queue_items, 1,
        "shared payload cords use exact capacity one"
    );
    assert!(matches!(args.fanout_branches, 2 | 8 | 32));
    assert!(matches!(args.fanout_mode, FanoutPublication::Coupled));
    assert!(matches!(args.payload_bytes, 1024 | 1_048_576));
    if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        assert!(matches!(
            args.termination_request,
            TerminationRequest::Complete
        ));
        assert_eq!(args.watch_slots, 0);
    }
    assert!(
        args.watch_slots == 0 || args.watch_slots == 1 || args.watch_slots == args.fanout_branches
    );
    if args.watch_slots == 0 {
        assert_eq!(args.watch_preview_bytes, 0);
    } else {
        assert_eq!(args.watch_preview_bytes, 64);
    }
    assert_eq!(args.slow_consumer_yields, 0);
    assert_eq!(args.cancel_after_offers, 0);
    assert_eq!(args.session_pump_quantum, 0);
    assert_eq!(args.residency_plateau_after_wakes, 0);
    assert_eq!(args.timer_advance_ticks, 0);
    assert!(matches!(
        args.termination_request,
        TerminationRequest::Complete | TerminationRequest::Abort
    ));
    assert!(matches!(args.session_mode, SessionMode::Finite));
    assert!(!args.identity_loop);

    let assembly_started = Instant::now();
    let payload_bytes = usize::try_from(args.payload_bytes).unwrap();
    let (source, payload) =
        shared_payload_source(args.fanout_branches, payload_bytes, args.payload_binding);
    let panel = conduit_panel::parse(&source).unwrap();
    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).unwrap();
    let topology = resolved.exact_topology().unwrap();
    let assembly_ns = assembly_started.elapsed().as_nanos() as u64;

    let seal_started = Instant::now();
    // ExactPlanDocument does not yet carry PlanFanOut. Seal the production
    // provider/profile skeleton, then assemble the current core fan-out fact
    // against the full source-derived topology without claiming compiler
    // support that does not exist.
    let (profile_source, _) = shared_payload_source(1, payload_bytes, args.payload_binding);
    let plan_memory_multiplier = if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        40 + 10 * u64::from(args.fanout_branches)
    } else {
        64 + 6 * u64::from(args.fanout_branches)
    };
    let plan_memory_bytes = args
        .payload_bytes
        .checked_mul(plan_memory_multiplier)
        .unwrap()
        .max(4 * 1024 * 1024);
    let mut host_observation = InstalledHostObservationInput::conduct_host();
    host_observation.available.memory_bytes = plan_memory_bytes;
    if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        let scratch_bytes = u32::try_from(args.payload_bytes.checked_mul(2).unwrap()).unwrap();
        for lane in &mut host_observation.execution_lanes {
            lane.scratch_bytes = scratch_bytes;
        }
    }
    let mut installed = InstalledProfile::observe_registry_on_host(
        &profile_source,
        &registry,
        &host_observation,
        &[],
    )
    .unwrap();
    installed.input.plan_budget.memory_bytes = plan_memory_bytes;
    if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        installed.input.plan_budget.cpu_units = 1 + 2 * u32::from(args.fanout_branches);
    }
    installed.input.plan_budget.evidence_bytes = if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        1024 * 1024
    } else {
        256 * 1024
    };
    installed.input.seal().unwrap();
    let document = compile_source(&profile_source, &installed.input).unwrap();
    let arena = Bump::new();
    let mut plan = document.as_plan(&arena).unwrap();
    let source_node = *plan
        .nodes
        .iter()
        .find(|node| node.instance.as_str() == "root/source")
        .unwrap();
    let sink_template = *plan
        .nodes
        .iter()
        .find(|node| node.instance.as_str() == "root/sink_0")
        .unwrap();
    let copy_template = plan
        .nodes
        .iter()
        .find(|node| node.instance.as_str() == "root/copy_0")
        .copied();
    let source_cord_template = *plan
        .cords
        .iter()
        .find(|cord| cord.from.node.as_str() == "root/source")
        .unwrap();
    let copy_cord_template = plan
        .cords
        .iter()
        .find(|cord| cord.from.node.as_str() == "root/copy_0")
        .copied();
    let nodes_per_branch = if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        2
    } else {
        1
    };
    let cords_per_branch = nodes_per_branch;
    let mut nodes = Vec::with_capacity(usize::from(args.fanout_branches) * nodes_per_branch + 1);
    let mut cords = Vec::with_capacity(usize::from(args.fanout_branches) * cords_per_branch);
    let mut branch_ids = Vec::with_capacity(usize::from(args.fanout_branches));
    nodes.push(source_node);
    for branch in 0..args.fanout_branches {
        let sink_instance = InstancePath::new(leaked(format!("root/sink_{branch}"))).unwrap();
        let copy_instance = matches!(
            args.payload_binding,
            PayloadBinding::BranchLocalUppercaseCopy
        )
        .then(|| InstancePath::new(leaked(format!("root/copy_{branch}"))).unwrap());
        if let Some(instance) = copy_instance {
            nodes.push(ResolvedPlanNode {
                instance,
                ..copy_template.unwrap()
            });
        }
        nodes.push(ResolvedPlanNode {
            instance: sink_instance,
            ..sink_template
        });
        let source_destination = copy_instance
            .map(|instance| instance.as_str())
            .unwrap_or(sink_instance.as_str());
        let topology_source_cord = topology
            .cords
            .iter()
            .find(|cord| cord.from_node == "root/source" && cord.to_node == source_destination)
            .unwrap();
        let source_cord_id = Id(leaked(topology_source_cord.id.clone()));
        branch_ids.push(source_cord_id);
        cords.push(ResolvedPlanCord {
            id: source_cord_id,
            to: ResolvedPlanPort {
                node: copy_instance.unwrap_or(sink_instance),
                ..source_cord_template.to
            },
            ..source_cord_template
        });
        if let Some(copy_instance) = copy_instance {
            let topology_copy_cord = topology
                .cords
                .iter()
                .find(|cord| {
                    cord.from_node == copy_instance.as_str()
                        && cord.to_node == sink_instance.as_str()
                })
                .unwrap();
            cords.push(ResolvedPlanCord {
                id: Id(leaked(topology_copy_cord.id.clone())),
                from: ResolvedPlanPort {
                    node: copy_instance,
                    ..copy_cord_template.unwrap().from
                },
                to: ResolvedPlanPort {
                    node: sink_instance,
                    ..copy_cord_template.unwrap().to
                },
                ..copy_cord_template.unwrap()
            });
        }
    }
    plan.nodes = arena.alloc_slice_copy(&nodes);
    plan.cords = arena.alloc_slice_copy(&cords);
    plan.fanouts = arena.alloc_slice_copy(&[PlanFanOut {
        id: Id("benchmark/payload/fanout"),
        producer: cords[0].from,
        mode: FanOutMode::Coupled,
        branches: arena.alloc_slice_copy(&branch_ids),
        duplicator: None,
        duplicator_input: None,
        duplication: DuplicationRule::SharedHandle,
    }]);
    let watch_ids = (0..args.watch_slots)
        .map(|watch| leaked(format!("watch/shared-payload-{watch}")))
        .collect::<Vec<_>>();
    let watch_leases = (0..args.watch_slots)
        .map(|watch| leaked(format!("lease/shared-payload-{watch}")))
        .collect::<Vec<_>>();
    let representation = PinnedDescriptor {
        id: cords[0].from.value_type.contract_id,
        schema_version: cords[0].from.value_type.schema_version,
        semantic_hash: cords[0].from.value_type.semantic_hash,
    };
    let watch_admissions = (0..usize::from(args.watch_slots))
        .map(|watch| WatchAdmission {
            id: Id(watch_ids[watch]),
            subject: WatchSubject::Cord(branch_ids[watch]),
            operator: Id("operator/benchmark"),
            control_grant_hash: SemanticHash::from_bytes([91; 32]),
            lease: Id(watch_leases[watch]),
            representation,
            maximum_preview_bytes: args.watch_preview_bytes,
            maximum_history: 1,
            minimum_tick_interval: 1,
            retention: WatchRetention::Latest,
            sensitivity_ceiling: Sensitivity::Public,
            reveal_action: None,
            reveal_grant_hash: None,
        })
        .collect::<Vec<_>>();
    plan.watch_admissions = arena.alloc_slice_copy(&watch_admissions);
    plan.source_semantic_hash = topology.source_semantic_hash;
    plan.identity = SemanticHash::from_bytes([0; 32]);
    let mut identity_scratch =
        vec![SemanticHash::from_bytes([0; 32]); plan.identity_fact_count().unwrap()];
    plan.identity = plan.semantic_hash(&mut identity_scratch).unwrap();
    validate_execution_plan(
        &plan,
        PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        &mut identity_scratch,
    )
    .unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let grant_observations = installed.grant_observations(&plan).unwrap();
    let use_observations = hosted_service_use_observations(&grant_observations);
    let seal_ns = seal_started.elapsed().as_nanos() as u64;
    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let policy = SchedulerPolicy {
        schema_version: SCHEDULER_CONTRACT_VERSION,
        ready_queue: ReadyQueueDiscipline::RoundRobin,
        max_decisions: u64::from(args.fanout_branches) * 128,
        max_tick: u64::from(args.fanout_branches) * 256,
        max_consecutive_yields: 8,
        max_events: u32::from(args.fanout_branches) * 32,
    };
    let reservation = SchedulerReservation {
        available_runtime_memory_bytes: plan.budget.memory_bytes,
        executor_overhead_limit_bytes: plan.budget.memory_bytes,
    };
    let start_started = Instant::now();
    let mut session = resolved
        .start_exact_session(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 1,
                run_id: Id("benchmark/shared-payload-fanout"),
                grant_observations: &grant_observations,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: policy,
                reservation,
            },
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();
    let start_ns = start_started.elapsed().as_nanos() as u64;
    let exact_identity = session.identity().clone();
    let watch_authority = |watch: usize, operation| ExactWatchUseAuthority {
        operation,
        operator_id: "operator/benchmark".to_owned(),
        control_grant_hash: SemanticHash::from_bytes([91; 32]),
        control_grant_active: true,
        run_id: exact_identity.run_id.clone(),
        plan_epoch: exact_identity.plan_epoch,
        watch_id: watch_ids[watch].to_owned(),
        lease_id: watch_leases[watch].to_owned(),
        lease_epoch: exact_identity.plan_epoch,
        lease_available: true,
        reveal_grant_hash: None,
        reveal_grant_active: false,
        time_basis: "clock/conduct-host".to_owned(),
        validated_at_tick: 12,
        valid_until_tick: u64::MAX,
    };
    for (watch, watch_id) in watch_ids.iter().enumerate() {
        session
            .attach_watch(
                watch_id,
                &watch_authority(watch, ExactWatchOperation::Attach),
            )
            .unwrap();
    }
    let allocation = session.allocation();
    let reserved_session_bytes = session.reserved_session_bytes();
    let host_io_capacity_bytes = session.with_io(ExactRunIo::capacity_bytes);
    let resident_before = proc_status_bytes("VmRSS:");
    let cpu_before = process_cpu_ns();
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::SeqCst);
    let steady_started = Instant::now();
    let mut pumps = 0_u64;
    let mut abort_ns = None;
    let mut pressured_items_at_stop = None;
    if matches!(args.termination_request, TerminationRequest::Abort) {
        session.pump(1, &use_observations).unwrap();
        pumps += 1;
        let published = session
            .value_storage_usage()
            .expect("hosted production drivers expose the fixed value arena");
        assert_eq!(published.resident_slots, 1);
        assert_eq!(published.resident_bytes, args.payload_bytes);
        assert_eq!(
            session.high_water().queue_items,
            u64::from(args.fanout_branches)
        );
        pressured_items_at_stop = Some(session.high_water().queue_items);
        let abort_started = Instant::now();
        session.cancel(StopPolicy::Abort).unwrap();
        abort_ns = Some(abort_started.elapsed().as_nanos() as u64);
    }
    while matches!(session.state(), ExactRunState::Active) {
        if let Err(error) = session.pump(512, &use_observations) {
            panic!(
                "{error:?}; state={:?}; high_water={:?}; pumps={pumps}",
                session.state(),
                session.high_water()
            );
        }
        pumps += 1;
    }
    let steady_ns = steady_started.elapsed().as_nanos() as u64;
    MEASURING.store(false, Ordering::SeqCst);
    let expected_terminal = if matches!(args.termination_request, TerminationRequest::Abort) {
        conduit_core::TerminalClass::Cancelled
    } else {
        conduit_core::TerminalClass::Succeeded
    };
    assert_eq!(session.state(), ExactRunState::Terminal(expected_terminal));
    let high_water = session.high_water();
    let value_usage = session
        .value_storage_usage()
        .expect("hosted production drivers expose the fixed value arena");
    let watch_usage = session.watch_usage();
    assert_eq!(watch_usage.admitted_slots, u32::from(args.watch_slots));
    assert_eq!(watch_usage.attached_slots, u32::from(args.watch_slots));
    assert_eq!(
        watch_usage.retained_observations,
        u64::from(args.watch_slots)
    );
    assert_eq!(
        watch_usage.retained_preview_bytes,
        u64::from(args.watch_slots) * u64::from(args.watch_preview_bytes)
    );
    assert_eq!(watch_usage.dropped_observations, 0);
    assert_eq!(
        watch_usage.maximum_observations,
        u64::from(args.watch_slots)
    );
    assert_eq!(
        watch_usage.maximum_preview_bytes,
        u64::from(args.watch_slots) * u64::from(args.watch_preview_bytes)
    );
    let display = session.with_io(|io| io.display().to_vec());
    let expected_branch_payload = if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        payload.to_uppercase()
    } else {
        payload.clone()
    };
    if matches!(args.termination_request, TerminationRequest::Abort) {
        assert!(display.is_empty());
    } else {
        assert_eq!(
            display,
            expected_branch_payload
                .repeat(usize::from(args.fanout_branches))
                .as_bytes()
        );
    }
    let host_io_output_bytes = u64::try_from(display.len()).unwrap();
    let expected_content_hash = SemanticHash::from_bytes(Sha256::digest(payload.as_bytes()).into());
    let expected_preview =
        payload.as_bytes()[..usize::try_from(args.watch_preview_bytes).unwrap()].to_vec();
    let mut watched_handles = BTreeSet::new();
    for (watch, watch_id) in watch_ids.iter().enumerate() {
        let batch = session
            .read_watch(
                watch_id,
                0,
                1,
                &watch_authority(watch, ExactWatchOperation::Read),
            )
            .unwrap();
        assert_eq!(batch.status, EvidenceCursorStatus::Available);
        assert_eq!(batch.records.len(), 1);
        let record = &batch.records[0];
        assert_eq!(record.original_bytes, args.payload_bytes as u32);
        assert_eq!(record.content_hash, Some(expected_content_hash));
        assert_eq!(
            record.material,
            ExactWatchMaterial::Preview(expected_preview.clone())
        );
        assert!(record.truncated);
        watched_handles.insert(record.value_handle);
    }
    let mut handles = BTreeSet::new();
    let mut branch_deliveries = 0_u64;
    let mut maximum_cord_items = 0_u16;
    for event in session.scheduler_events() {
        if let SchedulerSubject::Cord(cord) = event.subject {
            maximum_cord_items = maximum_cord_items.max(event.occupancy_items);
            let cord = &plan.cords[usize::from(cord)];
            if let Some(handle) = event.value_handle {
                handles.insert(handle);
            }
            if let Some(handle) = event.related_value_handle {
                handles.insert(handle);
            }
            if matches!(event.kind, SchedulerEventKind::ValueConsumed)
                && cord.to.node.as_str().starts_with("root/sink_")
            {
                branch_deliveries += 1;
            }
        }
    }
    let expected_handles = if matches!(
        args.payload_binding,
        PayloadBinding::BranchLocalUppercaseCopy
    ) {
        usize::from(args.fanout_branches) + 1
    } else {
        1
    };
    assert_eq!(handles.len(), expected_handles);
    if args.watch_slots == 0 {
        assert!(watched_handles.is_empty());
    } else {
        assert_eq!(watched_handles, handles);
    }
    let expected_deliveries = if matches!(args.termination_request, TerminationRequest::Abort) {
        0
    } else {
        u64::from(args.fanout_branches)
    };
    assert_eq!(branch_deliveries, expected_deliveries);
    assert!(maximum_cord_items <= args.queue_items);
    assert_eq!(value_usage.resident_slots, 0);
    assert_eq!(value_usage.resident_bytes, 0);
    if matches!(args.payload_binding, PayloadBinding::SharedHandle) {
        assert_eq!(value_usage.high_water_slots, 1);
        assert_eq!(value_usage.high_water_bytes, args.payload_bytes);
    } else {
        assert!(value_usage.high_water_slots >= 2);
        assert!(value_usage.high_water_slots <= u32::from(args.fanout_branches) + 1);
        assert!(value_usage.high_water_bytes >= args.payload_bytes * 2);
        assert!(
            value_usage.high_water_bytes
                <= args.payload_bytes * (u64::from(args.fanout_branches) + 1)
        );
    }
    MEASURING.store(true, Ordering::SeqCst);
    session.finalize().unwrap();
    MEASURING.store(false, Ordering::SeqCst);
    assert_eq!(sessions.active_sessions(), 0);
    assert_eq!(sessions.reserved_bytes(), 0);
    let cpu_ns = process_cpu_ns()
        .zip(cpu_before)
        .map(|(after, before)| after - before);

    RawSample {
        schema: "conduit.comparative-raw-sample",
        schema_version: 0,
        fixture_revision: 0,
        runtime: RuntimeConfiguration {
            id: "conduit-hosted-value-arena",
            comparison_role: "reactive-runtime",
            execution_mode: "production-hosted-exact-session",
            build_profile: "release",
            scheduler: "round-robin-bounded",
            fusion: "disabled",
            batching: if matches!(args.payload_binding, PayloadBinding::SharedHandle) {
                "one-shared-value"
            } else {
                "one-shared-source-value-plus-one-copy-per-branch"
            },
            concurrency: 1,
        },
        workload: WorkloadIdentity {
            id: args.workload,
            operators: 1,
            input_values: 1,
            queue_capacity_items: 1,
            ordering: "one source value reaches every branch in branch-number order",
            pressure: if matches!(args.payload_binding, PayloadBinding::SharedHandle) {
                "one production finite-batch source handle across capacity-one cords"
            } else {
                "one production source handle across capacity-one input cords; each branch produces one copied uppercase output handle"
            },
            terminal: if matches!(args.termination_request, TerminationRequest::Abort) {
                "Abort after atomic publication and before branch consumption"
            } else if matches!(
                args.payload_binding,
                PayloadBinding::BranchLocalUppercaseCopy
            ) {
                "complete after every production branch copy reaches its sink"
            } else {
                "complete after every branch consumes the shared handle"
            },
            loss: if matches!(args.termination_request, TerminationRequest::Abort) {
                "one admitted shared value cancelled before branch consumption"
            } else {
                "none"
            },
            slow_consumer_yields: 0,
            recovery_after_outputs: 0,
            fanout_branches: args.fanout_branches,
            fanout_mode: "coupled",
            slow_branches: "none",
            termination_request: args.termination_request.as_str(),
            cancel_after_offers: u64::from(matches!(
                args.termination_request,
                TerminationRequest::Abort
            )),
            consumer_pattern: "none",
            consumer_burst_items: 0,
            session_mode: "finite-exact-run-session",
            session_pump_quantum: 512,
            residency_plateau_after_wakes: 0,
            timer_advance_ticks: 0,
            payload_bytes: args.payload_bytes,
            payload_representation: args.payload_binding.representation(),
            watch_slots: args.watch_slots,
            watch_preview_bytes: args.watch_preview_bytes,
            watch_retention: if args.watch_slots == 0 {
                "none"
            } else {
                "latest"
            },
        },
        exact_identity: ExactIdentity {
            logical_fixture: format!(
                "comparative-shared-payload-fanout/{}/{}/{}/{}/{}/{}/{}",
                args.fanout_branches,
                args.payload_bytes,
                args.payload_binding.representation(),
                args.queue_items,
                args.termination_request.as_str(),
                args.watch_slots,
                args.watch_preview_bytes
            ),
            plan_identity: Some(plan.identity.to_string()),
            source_semantic_hash: Some(plan.source_semantic_hash.to_string()),
            artifact_digest: Some(plan.artifacts[0].digest.to_string()),
        },
        sample_kind,
        trial,
        thermal_state,
        phases: PhaseTimes {
            assembly_ns,
            plan_seal_ns: Some(seal_ns),
            start_ns: Some(start_ns),
            steady_ns,
            pressure_ns: None,
            recovery_ns: None,
            pressure_cycles: None,
            recovery_cycles: None,
        },
        execution: ExecutionMeasurement {
            scheduler_decisions: Some(high_water.decisions),
            producer_stall_ns: Some(0),
            drain_ns: None,
            abort_ns,
            session_pumps: Some(pumps),
            session_reserved_bytes: Some(reserved_session_bytes),
            pressured_items_at_stop,
            session_host_wakes: None,
            session_timer_wakes: None,
            residency_plateau_verified: None,
            residency_checkpoint_queue_items_high_water: None,
            residency_checkpoint_queue_payload_bytes_high_water: None,
            residency_checkpoint_ready_slots_high_water: None,
            residency_checkpoint_evidence_slots_high_water: None,
            unique_value_handles: Some(u64::try_from(handles.len()).unwrap()),
            branch_deliveries: Some(branch_deliveries),
            shared_handle_publications: Some(u64::from(args.fanout_branches)),
            payload_copy_operations: Some(
                if matches!(
                    args.payload_binding,
                    PayloadBinding::BranchLocalUppercaseCopy
                ) {
                    u64::from(args.fanout_branches)
                } else {
                    0
                },
            ),
            payload_bytes_copied: Some(
                if matches!(
                    args.payload_binding,
                    PayloadBinding::BranchLocalUppercaseCopy
                ) {
                    args.payload_bytes * u64::from(args.fanout_branches)
                } else {
                    0
                },
            ),
        },
        process_cpu_ns: cpu_ns,
        outcomes: OutcomeMeasurement {
            offered: 1,
            admitted: 1,
            completed_useful: branch_deliveries,
            rejected: 0,
            sampled: 0,
            coalesced: 0,
            dropped: 0,
            cancelled: u64::from(matches!(
                args.termination_request,
                TerminationRequest::Abort
            )),
            retried: 0,
            terminal: 1,
        },
        allocations: AllocationMeasurement {
            scope: "after-start scheduler execution and finalization; caller content and Watch-read verification excluded",
            calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
            bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        },
        memory: MemoryMeasurement {
            resident_before_bytes: resident_before,
            resident_after_bytes: proc_status_bytes("VmRSS:"),
            resident_peak_bytes: proc_status_bytes("VmHWM:"),
            planned_memory_bytes: Some(allocation.planned_memory_bytes),
            executor_overhead_bytes: Some(allocation.executor_overhead_bytes),
            queue_items_high_water: Some(high_water.queue_items),
            queue_max_cord_items_high_water: Some(maximum_cord_items),
            queue_payload_bytes_high_water: Some(high_water.queue_payload_bytes),
            ready_slots_high_water: Some(high_water.ready_slots),
            evidence_slots_high_water: Some(high_water.event_slots),
            value_resident_slots_after_terminal: Some(value_usage.resident_slots),
            value_resident_bytes_after_terminal: Some(value_usage.resident_bytes),
            value_slots_high_water: Some(value_usage.high_water_slots),
            value_bytes_high_water: Some(value_usage.high_water_bytes),
            value_slots_capacity: Some(value_usage.maximum_slots),
            value_bytes_capacity: Some(value_usage.maximum_bytes),
            host_io_capacity_bytes: Some(host_io_capacity_bytes),
            host_io_output_bytes: Some(host_io_output_bytes),
            watch_admitted_slots: Some(watch_usage.admitted_slots),
            watch_attached_slots: Some(watch_usage.attached_slots),
            watch_retained_observations: Some(watch_usage.retained_observations),
            watch_retained_preview_bytes: Some(watch_usage.retained_preview_bytes),
            watch_dropped_observations: Some(watch_usage.dropped_observations),
            watch_maximum_observations: Some(watch_usage.maximum_observations),
            watch_maximum_preview_bytes: Some(watch_usage.maximum_preview_bytes),
        },
        latency: LatencyMeasurement {
            clock: "CLOCK_MONOTONIC via std::time::Instant",
            sample_stride: 1,
            samples_ns: vec![steady_ns.max(1)],
        },
        semantic_notes: if matches!(
            args.payload_binding,
            PayloadBinding::BranchLocalUppercaseCopy
        ) {
            [
                "The benchmark harness assembles the current exact coupled shared-handle PlanFanOut for the source boundary; each branch then executes the production text/uppercase driver and stores one distinct copied handle before its display sink.",
                "Copy counts and bytes are exact from the one-copy-per-branch graph, after-Start allocator calls include the production uppercase buffers, full uppercase content verification and event-handle inspection remain outside the timed region, and this row does not claim execution of DuplicationRule::Copy.",
            ]
        } else if matches!(args.termination_request, TerminationRequest::Abort)
            && args.watch_slots > 0
        {
            [
                "The benchmark harness assembles the current exact coupled PlanFanOut plus pre-Start exact Watch admissions for the full source topology; the production hosted literal publishes one generation-safe handle across every capacity-one output cord.",
                "Abort follows atomic publication; fixed Latest previews retain verified 64-byte copies after terminal cleanup reclaims the one exact executor value, while Watch reads and caller verification remain outside the timed allocation scope.",
            ]
        } else if matches!(args.termination_request, TerminationRequest::Abort) {
            [
                "The benchmark harness assembles the current exact coupled PlanFanOut fact for the full source topology; the production hosted literal then stores one finite-batch value in the fixed arena and publishes its generation-safe handle across every capacity-one output cord.",
                "Abort is requested after atomic publication and before any sink consumes; terminal cleanup must reclaim the one exact value while queue charges remain visible and verifier output stays empty.",
            ]
        } else if args.watch_slots > 0 {
            [
                "The benchmark harness assembles the current exact coupled PlanFanOut plus pre-Start exact Watch admissions for the full source topology; the production hosted literal publishes one generation-safe handle across every capacity-one output cord.",
                "Every display branch consumes the shared handle; fixed Latest previews retain separately accounted verified 64-byte copies while terminal executor value residency remains zero and Watch reads stay outside the timed allocation scope.",
            ]
        } else {
            [
                "The benchmark harness assembles the current exact coupled PlanFanOut fact for the full source topology; the production hosted literal then stores one finite-batch value in the fixed arena and publishes its generation-safe handle across every capacity-one output cord.",
                "Exactly one topology-sized value slot is resident at high water; every display sink verifies its branch through separately accounted preallocated host-I/O storage, while payloads above the reviewed production ceiling and stream-specific fan-out modes remain unavailable.",
            ]
        },
    }
}

fn run_sample(
    args: &Args,
    sample_kind: &'static str,
    trial: u32,
    thermal_state: &'static str,
) -> RawSample {
    let prepared = prepare(args);
    let scheduled = prepared
        .drivers
        .into_iter()
        .zip(prepared.plan.nodes)
        .map(|(driver, node)| ScheduledNode {
            driver,
            machine: machine(node.execution_profile.unwrap(), node),
        })
        .collect();
    let decisions_per_value = if matches!(args.workload, Workload::Fanout) {
        u64::from(args.fanout_branches)
            .saturating_mul(u64::from(args.slow_consumer_yields) + 4)
            .saturating_add(16)
    } else {
        u64::try_from(args.operators + 9).unwrap()
    };
    let policy = SchedulerPolicy {
        schema_version: SCHEDULER_CONTRACT_VERSION,
        ready_queue: ReadyQueueDiscipline::RoundRobin,
        max_decisions: args.values.saturating_mul(decisions_per_value),
        max_tick: args
            .values
            .saturating_mul(decisions_per_value.saturating_add(1).saturating_add(
                if matches!(args.workload, Workload::PersistentTimer) {
                    args.timer_advance_ticks
                } else {
                    0
                },
            )),
        max_consecutive_yields: 8,
        max_events: 1024,
    };
    let reservation = SchedulerReservation {
        available_runtime_memory_bytes: prepared.plan.budget.memory_bytes,
        executor_overhead_limit_bytes: 256 * 1024 * 1024,
    };
    let start_started = Instant::now();
    let executor = DeterministicExecutor::start(
        prepared.plan,
        PlanValidationContext {
            supported_schema_version: prepared.plan.schema_version,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        },
        policy,
        reservation,
        scheduled,
    )
    .unwrap();
    let allocation = executor.allocation();
    let mut executor = Some(executor);
    let mut session_registry = None;
    let mut persistent_session = None;
    let mut session_reserved_bytes = None;
    if matches!(args.session_mode, SessionMode::Persistent) {
        let sessions =
            ExactRunSessionRegistry::new(1, reservation.available_runtime_memory_bytes).unwrap();
        let admission = sessions
            .admit(reservation.available_runtime_memory_bytes)
            .unwrap();
        session_reserved_bytes = Some(reservation.available_runtime_memory_bytes);
        let identity = ExactRunIdentity {
            plan_identity: prepared.plan.identity,
            source_semantic_hash: prepared.plan.source_semantic_hash,
            plan_epoch: 1,
            run_id: match args.workload {
                Workload::PersistentWake => "benchmark/persistent-wake",
                Workload::PersistentTimer => "benchmark/persistent-timer",
                _ => "benchmark/persistent-overload",
            }
            .to_owned(),
        };
        persistent_session = Some(ExactRunSession::new(
            admission,
            identity,
            executor.take().unwrap(),
        ));
        session_registry = Some(sessions);
    }
    let start_ns = start_started.elapsed().as_nanos() as u64;
    let resident_before = proc_status_bytes("VmRSS:");
    let cpu_before = process_cpu_ns();
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::SeqCst);
    let steady_started = Instant::now();
    let mut execution_error = None;
    let mut requested_stop_started = None;
    let mut session_pumps = None;
    let mut pressured_items_at_stop = None;
    let mut session_host_wakes = None;
    let mut session_timer_wakes = None;
    let mut residency_plateau_verified = None;
    let mut residency_checkpoint_queue_items_high_water = None;
    let mut residency_checkpoint_queue_payload_bytes_high_water = None;
    let mut residency_checkpoint_ready_slots_high_water = None;
    let mut residency_checkpoint_evidence_slots_high_water = None;
    let (status, executor) = if matches!(args.session_mode, SessionMode::Persistent) {
        let mut session = persistent_session.take().unwrap();
        let mut pumps = 0_u64;
        let status = if matches!(
            args.workload,
            Workload::PersistentWake | Workload::PersistentTimer
        ) {
            loop {
                let pump = session.pump(args.session_pump_quantum).unwrap();
                pumps += 1;
                if session.scheduler_event_count() >= 768 {
                    session
                        .acknowledge_scheduler_events_through(session.next_event_cursor())
                        .unwrap();
                }
                if matches!(pump.state, conduit_runtime::ExactRunState::Waiting) {
                    assert!(prepared.observations.observation_window_waiting.get());
                    break;
                }
            }

            let mut plateau = None;
            for wake in 1..=args.values {
                assert!(prepared.observations.observation_window_waiting.get());
                prepared.observations.observation_window_waiting.set(false);
                match args.workload {
                    Workload::PersistentWake => {
                        session
                            .notify_host_operation(Id("benchmark/persistent-wake"))
                            .unwrap();
                    }
                    Workload::PersistentTimer => {
                        let deadline = session
                            .next_timer_deadline()
                            .expect("persistent timer retains one exact deadline");
                        session.advance_to(deadline).unwrap();
                    }
                    _ => unreachable!(),
                }
                loop {
                    let pump = session.pump(args.session_pump_quantum).unwrap();
                    pumps += 1;
                    if session.scheduler_event_count() >= 768 {
                        session
                            .acknowledge_scheduler_events_through(session.next_event_cursor())
                            .unwrap();
                    }
                    if matches!(pump.state, conduit_runtime::ExactRunState::Waiting) {
                        assert!(prepared.observations.observation_window_waiting.get());
                        assert_eq!(prepared.observations.offered.get(), wake);
                        assert_eq!(prepared.observations.accepted_values.get(), wake);
                        assert_eq!(prepared.observations.useful_outputs.get(), wake);
                        break;
                    }
                    assert!(matches!(
                        session.scheduler_status(),
                        SchedulerStatus::Running | SchedulerStatus::Stalled
                    ));
                }
                if wake == args.residency_plateau_after_wakes {
                    plateau = Some(session.high_water());
                }
            }
            let plateau = plateau.expect("residency checkpoint was observed");
            let final_wake_high_water = session.high_water();
            assert_eq!(final_wake_high_water.queue_items, plateau.queue_items);
            assert_eq!(
                final_wake_high_water.queue_payload_bytes,
                plateau.queue_payload_bytes
            );
            assert_eq!(final_wake_high_water.ready_slots, plateau.ready_slots);
            assert_eq!(final_wake_high_water.event_slots, plateau.event_slots);
            if matches!(args.workload, Workload::PersistentWake) {
                session_host_wakes = Some(args.values);
            } else {
                session_timer_wakes = Some(args.values);
            }
            residency_plateau_verified = Some(true);
            residency_checkpoint_queue_items_high_water = Some(plateau.queue_items);
            residency_checkpoint_queue_payload_bytes_high_water = Some(plateau.queue_payload_bytes);
            residency_checkpoint_ready_slots_high_water = Some(plateau.ready_slots);
            residency_checkpoint_evidence_slots_high_water = Some(plateau.event_slots);
            requested_stop_started = Some(Instant::now());
            prepared.observations.terminal_requested.set(true);
            session
                .cancel(args.termination_request.stop_policy().unwrap())
                .unwrap();
            loop {
                let scheduler_status = session.scheduler_status();
                if !matches!(
                    scheduler_status,
                    SchedulerStatus::Running | SchedulerStatus::Stalled
                ) {
                    break Some(scheduler_status);
                }
                match session.pump(args.session_pump_quantum) {
                    Ok(_) => pumps += 1,
                    Err(error) => {
                        execution_error = Some(error);
                        break None;
                    }
                }
            }
        } else {
            loop {
                let pump = match session.pump(args.session_pump_quantum) {
                    Ok(pump) => pump,
                    Err(error) => {
                        execution_error = Some(error);
                        break None;
                    }
                };
                pumps += 1;
                if session.scheduler_event_count() >= 768 {
                    session
                        .acknowledge_scheduler_events_through(session.next_event_cursor())
                        .unwrap();
                }
                if requested_stop_started.is_none()
                    && prepared.observations.offered.get() >= args.cancel_after_offers
                    && (matches!(args.pressure_policy, PressurePolicy::Block)
                        || prepared.observations.observation_window_waiting.get())
                {
                    let pressured = prepared
                        .observations
                        .accepted_values
                        .get()
                        .saturating_sub(prepared.observations.coalesced.get())
                        .saturating_sub(prepared.observations.useful_outputs.get());
                    assert!(
                        pressured > 0,
                        "persistent terminal request must observe admitted work under pressure"
                    );
                    pressured_items_at_stop = Some(pressured);
                    requested_stop_started = Some(Instant::now());
                    prepared.observations.terminal_requested.set(true);
                    session
                        .cancel(args.termination_request.stop_policy().unwrap())
                        .unwrap();
                }
                let scheduler_status = session.scheduler_status();
                if !matches!(
                    scheduler_status,
                    SchedulerStatus::Running | SchedulerStatus::Stalled
                ) {
                    break Some(scheduler_status);
                }
                if matches!(pump.state, conduit_runtime::ExactRunState::Waiting)
                    && requested_stop_started.is_none()
                {
                    panic!("persistent session waited before its observation boundary");
                }
            }
        };
        session_pumps = Some(pumps);
        let executor = session
            .finalize()
            .expect("persistent session reached one terminal state");
        assert_eq!(session_registry.as_ref().unwrap().active_sessions(), 0);
        assert_eq!(session_registry.as_ref().unwrap().reserved_bytes(), 0);
        (status, executor)
    } else {
        let mut executor = executor.take().unwrap();
        let status = loop {
            let status = match executor.run_one() {
                Ok(status) => status,
                Err(error) => {
                    execution_error = Some(error);
                    break None;
                }
            };
            if executor.event_count() >= 768 {
                executor
                    .acknowledge_events_through(executor.next_event_cursor())
                    .unwrap();
            }
            if requested_stop_started.is_none()
                && prepared.observations.offered.get() >= args.cancel_after_offers
                && args.termination_request.stop_policy().is_some()
            {
                pressured_items_at_stop = Some(
                    prepared
                        .observations
                        .accepted_values
                        .get()
                        .saturating_sub(prepared.observations.coalesced.get())
                        .saturating_sub(prepared.observations.useful_outputs.get()),
                );
                requested_stop_started = Some(Instant::now());
                prepared.observations.terminal_requested.set(true);
                executor
                    .cancel(args.termination_request.stop_policy().unwrap())
                    .unwrap();
                if !matches!(executor.status(), SchedulerStatus::Running) {
                    break Some(executor.status());
                }
            }
            if !matches!(status, SchedulerStatus::Running) {
                break Some(status);
            }
        };
        (status, executor)
    };
    finish_producer_stall(&prepared.observations);
    let requested_stop_ns =
        requested_stop_started.map(|started| started.elapsed().as_nanos() as u64);
    let steady_ns = steady_started.elapsed().as_nanos() as u64;
    let recovery_ns = prepared
        .observations
        .recovery_started
        .borrow()
        .map(|started| started.elapsed().as_nanos() as u64);
    MEASURING.store(false, Ordering::SeqCst);
    let cpu_ns = process_cpu_ns()
        .zip(cpu_before)
        .map(|(after, before)| after - before);
    if args.termination_request.stop_policy().is_some() {
        assert!(
            execution_error.is_none(),
            "cancellation execution failed: {execution_error:?}"
        );
        assert_eq!(status, Some(SchedulerStatus::Cancelled));
    } else if matches!(args.workload, Workload::Overload)
        && matches!(args.pressure_policy, PressurePolicy::Fail)
    {
        assert!(execution_error.is_some());
    } else {
        assert!(execution_error.is_none());
        if matches!(args.workload, Workload::Overload)
            && matches!(args.pressure_policy, PressurePolicy::Disconnect)
        {
            assert_eq!(status, Some(SchedulerStatus::Disconnected));
        } else {
            assert_eq!(status, Some(SchedulerStatus::Succeeded));
        }
    }
    let high_water = executor.high_water();
    RawSample {
        schema: "conduit.comparative-raw-sample",
        schema_version: 0,
        fixture_revision: 0,
        runtime: RuntimeConfiguration {
            id: "conduit-reference-scheduler",
            comparison_role: "reactive-runtime",
            execution_mode: if matches!(args.session_mode, SessionMode::Persistent) {
                "persistent-session-bounded-pump-single-lane"
            } else {
                "deterministic-single-lane"
            },
            build_profile: "release",
            scheduler: "round-robin-bounded",
            fusion: "disabled",
            batching: "one-value-per-step",
            concurrency: 1,
        },
        workload: WorkloadIdentity {
            id: args.workload,
            operators: args.operators,
            input_values: args.values,
            queue_capacity_items: args.queue_items,
            ordering: if matches!(args.workload, Workload::Fanout) {
                "source order independently at every exact branch"
            } else if matches!(args.workload, Workload::PersistentWake) {
                "one host wake admits one source-ordered value"
            } else if matches!(args.workload, Workload::PersistentTimer) {
                "one exact timer wake admits one source-ordered value"
            } else {
                "source-order; merge uses retained round-robin"
            },
            pressure: if matches!(args.workload, Workload::Overload) {
                args.pressure_policy.as_str()
            } else if matches!(args.workload, Workload::PersistentWake) {
                "exact host wake to bounded FIFO"
            } else if matches!(args.workload, Workload::PersistentTimer) {
                "exact timer wake to bounded FIFO"
            } else if matches!(args.workload, Workload::Fanout) {
                if matches!(args.fanout_mode, FanoutPublication::Coupled) {
                    "atomic publication waits for every branch"
                } else {
                    "ordinary duplicator publishes independently to finite branch cords"
                }
            } else {
                "bounded FIFO block"
            },
            terminal: if args.termination_request.stop_policy().is_some() {
                match (args.workload, args.session_mode, args.termination_request) {
                    (Workload::PersistentWake, _, TerminationRequest::Drain) => {
                        "persistent host-wake session requested Drain after the final re-wait"
                    }
                    (Workload::PersistentTimer, _, TerminationRequest::Drain) => {
                        "persistent timer session requested Drain after the final re-wait"
                    }
                    (_, SessionMode::Persistent, TerminationRequest::Drain) => {
                        "persistent session requested Drain at the observation boundary"
                    }
                    (_, SessionMode::Persistent, TerminationRequest::Abort) => {
                        "persistent session requested Abort at the observation boundary"
                    }
                    (_, _, TerminationRequest::Drain) => "requested Drain while pressured",
                    (_, _, TerminationRequest::Abort) => "requested Abort while pressured",
                    (_, _, TerminationRequest::Complete) => unreachable!(),
                }
            } else if matches!(args.workload, Workload::Overload) {
                match args.pressure_policy {
                    PressurePolicy::Disconnect => "disconnect on first saturated offer",
                    PressurePolicy::Fail => "fail execution on first saturated offer",
                    _ => "complete after admitted values drain",
                }
            } else {
                "complete after all admitted values drain"
            },
            loss: if matches!(args.workload, Workload::Overload) {
                args.pressure_policy.loss()
            } else {
                "none"
            },
            slow_consumer_yields: if matches!(args.workload, Workload::Overload | Workload::Fanout)
            {
                args.slow_consumer_yields
            } else {
                0
            },
            recovery_after_outputs: recovery_after_outputs(args),
            fanout_branches: if matches!(args.workload, Workload::Fanout) {
                args.fanout_branches
            } else {
                1
            },
            fanout_mode: if matches!(args.workload, Workload::Fanout) {
                args.fanout_mode.as_str()
            } else {
                "none"
            },
            slow_branches: if matches!(args.workload, Workload::Fanout) {
                args.slow_branches.as_str()
            } else if matches!(args.workload, Workload::Overload) {
                "one"
            } else {
                "none"
            },
            termination_request: args.termination_request.as_str(),
            cancel_after_offers: args.cancel_after_offers,
            consumer_pattern: if matches!(args.workload, Workload::Overload | Workload::Fanout) {
                args.consumer_pattern.as_str()
            } else {
                "none"
            },
            consumer_burst_items: if matches!(args.workload, Workload::Overload | Workload::Fanout)
            {
                args.consumer_burst_items
            } else {
                0
            },
            session_mode: args.session_mode.as_str(),
            session_pump_quantum: args.session_pump_quantum,
            residency_plateau_after_wakes: args.residency_plateau_after_wakes,
            timer_advance_ticks: args.timer_advance_ticks,
            payload_bytes: 0,
            payload_representation: "handle-backed-u64",
            watch_slots: 0,
            watch_preview_bytes: 0,
            watch_retention: "none",
        },
        exact_identity: ExactIdentity {
            logical_fixture: if matches!(args.workload, Workload::Overload) {
                format!(
                    "comparative-overload/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}",
                    args.pressure_policy.id(),
                    args.termination_request.as_str(),
                    args.session_mode.as_str(),
                    args.session_pump_quantum,
                    args.consumer_pattern.as_str(),
                    args.consumer_burst_items,
                    args.values,
                    args.queue_items,
                    args.slow_consumer_yields,
                    args.cancel_after_offers,
                    args.latency_sample_stride
                )
            } else if matches!(args.workload, Workload::Fanout) {
                format!(
                    "comparative-fanout/{}/{}/{}/{}/{}/{}/{}/{}/{}",
                    args.fanout_mode.as_str(),
                    args.slow_branches.as_str(),
                    args.consumer_pattern.as_str(),
                    args.consumer_burst_items,
                    args.fanout_branches,
                    args.values,
                    args.queue_items,
                    args.slow_consumer_yields,
                    args.latency_sample_stride
                )
            } else if matches!(args.workload, Workload::PersistentWake) {
                format!(
                    "comparative-persistent-wake/{}/{}/{}/{}/{}",
                    args.values,
                    args.residency_plateau_after_wakes,
                    args.queue_items,
                    args.session_pump_quantum,
                    args.latency_sample_stride
                )
            } else if matches!(args.workload, Workload::PersistentTimer) {
                format!(
                    "comparative-persistent-timer/{}/{}/{}/{}/{}/{}",
                    args.values,
                    args.residency_plateau_after_wakes,
                    args.queue_items,
                    args.session_pump_quantum,
                    args.timer_advance_ticks,
                    args.latency_sample_stride
                )
            } else {
                format!(
                    "comparative-local-depth/{}/{}/{}/{}/{}",
                    args.workload.as_str(),
                    args.operators,
                    args.values,
                    args.queue_items,
                    args.latency_sample_stride
                )
            },
            plan_identity: Some(prepared.plan.identity.to_string()),
            source_semantic_hash: Some(prepared.plan.source_semantic_hash.to_string()),
            artifact_digest: Some(prepared.plan.artifacts[0].digest.to_string()),
        },
        sample_kind,
        trial,
        thermal_state,
        phases: PhaseTimes {
            assembly_ns: prepared.assembly_ns,
            plan_seal_ns: Some(prepared.seal_ns),
            start_ns: Some(start_ns),
            steady_ns,
            pressure_ns: recovery_ns.map(|recovery| steady_ns.saturating_sub(recovery)),
            recovery_ns,
            pressure_cycles: matches!(args.workload, Workload::Overload | Workload::Fanout).then(
                || {
                    if matches!(args.consumer_pattern, ConsumerPattern::Bursty) {
                        prepared.observations.pressure_cycles.get()
                    } else {
                        1
                    }
                },
            ),
            recovery_cycles: matches!(args.workload, Workload::Overload | Workload::Fanout).then(
                || {
                    if matches!(args.consumer_pattern, ConsumerPattern::Bursty) {
                        prepared.observations.recovery_cycles.get()
                    } else {
                        u64::from(recovery_ns.is_some())
                    }
                },
            ),
        },
        execution: ExecutionMeasurement {
            scheduler_decisions: Some(high_water.decisions),
            producer_stall_ns: Some(prepared.observations.producer_stall_ns.get()),
            drain_ns: matches!(args.termination_request, TerminationRequest::Drain)
                .then(|| requested_stop_ns.expect("Drain was requested")),
            abort_ns: matches!(args.termination_request, TerminationRequest::Abort)
                .then(|| requested_stop_ns.expect("Abort was requested")),
            session_pumps,
            session_reserved_bytes,
            pressured_items_at_stop,
            session_host_wakes,
            session_timer_wakes,
            residency_plateau_verified,
            residency_checkpoint_queue_items_high_water,
            residency_checkpoint_queue_payload_bytes_high_water,
            residency_checkpoint_ready_slots_high_water,
            residency_checkpoint_evidence_slots_high_water,
            unique_value_handles: None,
            branch_deliveries: None,
            shared_handle_publications: None,
            payload_copy_operations: None,
            payload_bytes_copied: None,
        },
        process_cpu_ns: cpu_ns,
        outcomes: OutcomeMeasurement {
            offered: prepared.observations.offered.get(),
            admitted: prepared.observations.accepted_values.get(),
            completed_useful: prepared.observations.useful_outputs.get(),
            rejected: prepared.observations.rejected.get(),
            sampled: prepared.observations.sampled.get(),
            coalesced: prepared.observations.coalesced.get(),
            dropped: prepared.observations.dropped.get(),
            cancelled: if args.termination_request.stop_policy().is_some() {
                prepared
                    .observations
                    .offered
                    .get()
                    .saturating_sub(prepared.observations.accepted_values.get())
                    .saturating_sub(prepared.observations.rejected.get())
                    .saturating_sub(prepared.observations.sampled.get())
                    .saturating_sub(prepared.observations.dropped.get())
            } else {
                0
            },
            retried: prepared.observations.retried.get(),
            terminal: 1,
        },
        allocations: AllocationMeasurement {
            scope: "after-start-through-terminal",
            calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
            bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        },
        memory: MemoryMeasurement {
            resident_before_bytes: resident_before,
            resident_after_bytes: proc_status_bytes("VmRSS:"),
            resident_peak_bytes: proc_status_bytes("VmHWM:"),
            planned_memory_bytes: Some(allocation.planned_memory_bytes),
            executor_overhead_bytes: Some(allocation.executor_overhead_bytes),
            queue_items_high_water: Some(high_water.queue_items),
            queue_max_cord_items_high_water: Some(executor.max_cord_occupancy()),
            queue_payload_bytes_high_water: Some(high_water.queue_payload_bytes),
            ready_slots_high_water: Some(high_water.ready_slots),
            evidence_slots_high_water: Some(high_water.event_slots),
            value_resident_slots_after_terminal: None,
            value_resident_bytes_after_terminal: None,
            value_slots_high_water: None,
            value_bytes_high_water: None,
            value_slots_capacity: None,
            value_bytes_capacity: None,
            host_io_capacity_bytes: None,
            host_io_output_bytes: None,
            watch_admitted_slots: None,
            watch_attached_slots: None,
            watch_retained_observations: None,
            watch_retained_preview_bytes: None,
            watch_dropped_observations: None,
            watch_maximum_observations: None,
            watch_maximum_preview_bytes: None,
        },
        latency: LatencyMeasurement {
            clock: "CLOCK_MONOTONIC via std::time::Instant",
            sample_stride: args.latency_sample_stride,
            samples_ns: prepared.observations.latencies.borrow().clone(),
        },
        semantic_notes: [
            "The public deterministic executor validates and preallocates the exact plan; timed execution does not disable contract checks.",
            if matches!(args.workload, Workload::PersistentWake) {
                "The production exact-run session reaches the same bounded queue, payload, ready, and evidence high-water values at the declared wake checkpoint and after the final host wake."
            } else if matches!(args.workload, Workload::PersistentTimer) {
                "The production exact-run session reaches the same bounded queue, payload, ready, and evidence high-water values at the declared wake checkpoint and after the final exact timer wake."
            } else if matches!(args.session_mode, SessionMode::Persistent) {
                "The production exact-run session owns one admitted reservation across bounded host pumps and releases it only after explicit terminal finalization."
            } else if matches!(args.workload, Workload::Overload) {
                "The pinned benchmark value type proves disposability and the exact latest-wins coalescer before the pressure plan is sealed."
            } else if matches!(args.workload, Workload::Fanout) {
                if matches!(args.fanout_mode, FanoutPublication::Coupled) {
                    "The exact plan pins coupled atomic copy publication; every branch has its own finite cord and no benchmark-side buffer."
                } else {
                    "The exact plan pins an ordinary isolated duplicator with one profile-accounted retained value and one finite cord per branch."
                }
            } else {
                "The handle-backed u64 fixture isolates scheduler cost and is not the optimized hosted streaming mode, which is not yet available."
            },
        ],
    }
}

fn run_identity_sample(
    args: &Args,
    sample_kind: &'static str,
    trial: u32,
    thermal_state: &'static str,
) -> RawSample {
    assert!(
        !matches!(
            args.workload,
            Workload::BoundedAsync
                | Workload::Overload
                | Workload::Fanout
                | Workload::SharedPayloadFanout
                | Workload::PersistentWake
                | Workload::PersistentTimer
        ),
        "an identity loop cannot model an asynchronous, overload, or fan-out boundary"
    );
    let assembly_started = Instant::now();
    let sample_count = usize::try_from(args.values.div_ceil(args.latency_sample_stride)).unwrap();
    let mut starts = vec![None; sample_count];
    let mut latencies = Vec::with_capacity(sample_count);
    let assembly_ns = assembly_started.elapsed().as_nanos() as u64;
    let resident_before = proc_status_bytes("VmRSS:");
    let cpu_before = process_cpu_ns();
    let transform_count = if matches!(args.workload, Workload::Merge) {
        args.operators.saturating_sub(1)
    } else {
        args.operators
    };
    let mut accepted_values = 0_u64;
    let mut useful_outputs = 0_u64;
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::SeqCst);
    let steady_started = Instant::now();
    for original in 0..args.values {
        accepted_values += 1;
        if original % args.latency_sample_stride == 0 {
            starts[usize::try_from(original / args.latency_sample_stride).unwrap()] =
                Some(Instant::now());
        }
        let mut value = original;
        let mut retained = true;
        for operator in 0..transform_count {
            if matches!(args.workload, Workload::MapFilter) && operator % 2 == 1 {
                retained = value % 2 == 0;
                if !retained {
                    break;
                }
            } else {
                value = black_box(value.wrapping_add(2));
            }
        }
        if retained {
            useful_outputs += 1;
            if original % args.latency_sample_stride == 0 {
                let sample = usize::try_from(original / args.latency_sample_stride).unwrap();
                latencies.push(starts[sample].unwrap().elapsed().as_nanos() as u64);
            }
            black_box(value);
        }
    }
    let steady_ns = steady_started.elapsed().as_nanos() as u64;
    MEASURING.store(false, Ordering::SeqCst);
    let cpu_ns = process_cpu_ns()
        .zip(cpu_before)
        .map(|(after, before)| after - before);
    RawSample {
        schema: "conduit.comparative-raw-sample",
        schema_version: 0,
        fixture_revision: 0,
        runtime: RuntimeConfiguration {
            id: "rust-identity-loop",
            comparison_role: "language-lower-bound",
            execution_mode: "single-threaded-for-loop",
            build_profile: "release",
            scheduler: "none",
            fusion: "not-applicable",
            batching: "none",
            concurrency: 1,
        },
        workload: WorkloadIdentity {
            id: args.workload,
            operators: args.operators,
            input_values: args.values,
            queue_capacity_items: 0,
            ordering: "ascending input order; merge boundary omitted",
            pressure: "not-applicable",
            terminal: "loop exhaustion",
            loss: "none",
            slow_consumer_yields: 0,
            recovery_after_outputs: 0,
            fanout_branches: 1,
            fanout_mode: "none",
            slow_branches: "none",
            termination_request: "complete",
            cancel_after_offers: 0,
            consumer_pattern: "none",
            consumer_burst_items: 0,
            session_mode: "finite-executor",
            session_pump_quantum: 0,
            residency_plateau_after_wakes: 0,
            timer_advance_ticks: 0,
            payload_bytes: 0,
            payload_representation: "native-u64",
            watch_slots: 0,
            watch_preview_bytes: 0,
            watch_retention: "none",
        },
        exact_identity: ExactIdentity {
            logical_fixture: format!(
                "comparative-local-depth/{}/{}/{}/{}/{}",
                args.workload.as_str(),
                args.operators,
                args.values,
                args.queue_items,
                args.latency_sample_stride
            ),
            plan_identity: None,
            source_semantic_hash: None,
            artifact_digest: None,
        },
        sample_kind,
        trial,
        thermal_state,
        phases: PhaseTimes {
            assembly_ns,
            plan_seal_ns: None,
            start_ns: None,
            steady_ns,
            pressure_ns: None,
            recovery_ns: None,
            pressure_cycles: None,
            recovery_cycles: None,
        },
        execution: ExecutionMeasurement {
            scheduler_decisions: None,
            producer_stall_ns: None,
            drain_ns: None,
            abort_ns: None,
            session_pumps: None,
            session_reserved_bytes: None,
            pressured_items_at_stop: None,
            session_host_wakes: None,
            session_timer_wakes: None,
            residency_plateau_verified: None,
            residency_checkpoint_queue_items_high_water: None,
            residency_checkpoint_queue_payload_bytes_high_water: None,
            residency_checkpoint_ready_slots_high_water: None,
            residency_checkpoint_evidence_slots_high_water: None,
            unique_value_handles: None,
            branch_deliveries: None,
            shared_handle_publications: None,
            payload_copy_operations: None,
            payload_bytes_copied: None,
        },
        process_cpu_ns: cpu_ns,
        outcomes: OutcomeMeasurement {
            offered: args.values,
            admitted: accepted_values,
            completed_useful: useful_outputs,
            rejected: 0,
            sampled: 0,
            coalesced: 0,
            dropped: 0,
            cancelled: 0,
            retried: 0,
            terminal: 1,
        },
        allocations: AllocationMeasurement {
            scope: "steady-loop",
            calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
            bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        },
        memory: MemoryMeasurement {
            resident_before_bytes: resident_before,
            resident_after_bytes: proc_status_bytes("VmRSS:"),
            resident_peak_bytes: proc_status_bytes("VmHWM:"),
            planned_memory_bytes: None,
            executor_overhead_bytes: None,
            queue_items_high_water: None,
            queue_max_cord_items_high_water: None,
            queue_payload_bytes_high_water: None,
            ready_slots_high_water: None,
            evidence_slots_high_water: None,
            value_resident_slots_after_terminal: None,
            value_resident_bytes_after_terminal: None,
            value_slots_high_water: None,
            value_bytes_high_water: None,
            value_slots_capacity: None,
            value_bytes_capacity: None,
            host_io_capacity_bytes: None,
            host_io_output_bytes: None,
            watch_admitted_slots: None,
            watch_attached_slots: None,
            watch_retained_observations: None,
            watch_retained_preview_bytes: None,
            watch_dropped_observations: None,
            watch_maximum_observations: None,
            watch_maximum_preview_bytes: None,
        },
        latency: LatencyMeasurement {
            clock: "CLOCK_MONOTONIC via std::time::Instant",
            sample_stride: args.latency_sample_stride,
            samples_ns: latencies,
        },
        semantic_notes: [
            "This no-framework Rust loop is a language-cost lower bound, not a reactive-runtime competitor.",
            "It has no plan, scheduler, cord, pressure, evidence, or merge boundary and cannot support runtime claims.",
        ],
    }
}

fn main() {
    let args = Args::parse();
    assert!(
        args.warmup_trials > 0,
        "at least one disclosed warm-up trial is required"
    );
    assert!(
        args.measured_trials > 0,
        "at least one measured trial is required"
    );
    for trial in 0..args.warmup_trials {
        let thermal_state = if trial == 0 { "cold" } else { "warming" };
        let raw = if matches!(args.workload, Workload::SharedPayloadFanout) {
            run_shared_payload_sample(&args, "warmup", trial, thermal_state)
        } else if args.identity_loop {
            run_identity_sample(&args, "warmup", trial, thermal_state)
        } else {
            run_sample(&args, "warmup", trial, thermal_state)
        };
        println!("{}", serde_json::to_string(&raw).unwrap());
    }
    for trial in 0..args.measured_trials {
        let raw = if matches!(args.workload, Workload::SharedPayloadFanout) {
            run_shared_payload_sample(&args, "measured", trial, "warmed")
        } else if args.identity_loop {
            run_identity_sample(&args, "measured", trial, "warmed")
        } else {
            run_sample(&args, "measured", trial, "warmed")
        };
        println!("{}", serde_json::to_string(&raw).unwrap());
    }
}
