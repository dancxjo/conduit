use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    hint::black_box,
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use clap::{Parser, ValueEnum};
use conduit_core::{
    ArtifactDigest, AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee,
    CompatibilityOutcome, Direction, DuplicationRule, ExecutionLimits, ExecutionPlan,
    ExecutionProfile, FanOutMode, FlowCapacity, FlowPolicy, FlowQueueState, FlowTypeFacts,
    FlowWatermarks, Id, ImplementationMachine, InstancePath, InstantiationContext, LifecycleUsage,
    MemoryAccounting, MemoryCategory, MemoryClaim, PinnedDescriptor, PlanArtifact, PlanFanOut,
    PlanHostObservation, PlanResourceBudget, PlanValidationContext, Pressure, ReadyQueueDiscipline,
    ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, SCHEDULER_CONTRACT_VERSION,
    SampleSchedule, SchedulerPolicy, SemanticHash, StopPolicy, TraitProof, TypeContractRef,
    validate_execution_plan,
};
use conduit_runtime::{
    DeterministicExecutor, RetainedValueUsage, RuntimeValue, RuntimeValueEnvelope, ScheduledNode,
    SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep, SendStatus, StepIo,
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
            matches!(args.workload, Workload::Overload),
            "the current cancellation fixture is the exact single-cord overload plan"
        );
        assert!(
            matches!(args.pressure_policy, PressurePolicy::Block),
            "cancellation is measured under FIFO block pressure"
        );
        assert!(
            args.cancel_after_offers > u64::from(args.queue_items)
                && args.cancel_after_offers < args.values,
            "cancellation must occur after pressure begins and before source completion"
        );
    } else {
        assert_eq!(
            args.cancel_after_offers, 0,
            "complete fixtures do not carry an unused cancellation threshold"
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
        stride: args.latency_sample_stride,
    };
    assert_eq!(observations.values.borrow().len(), value_count);

    let transform_count = match args.workload {
        Workload::Merge => args.operators.saturating_sub(1),
        Workload::Overload => 0,
        Workload::Fanout => 0,
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
        });
        drivers.push(BenchNode::Source {
            next: split,
            end: args.values,
            cord: 1,
            observations: observations.clone(),
            retrying: false,
            pressure: args.pressure_policy,
            queue_capacity: u64::from(args.queue_items),
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
            records_recovery: matches!(args.workload, Workload::Overload),
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
        max_tick: args.values.saturating_mul(decisions_per_value + 1),
        max_consecutive_yields: 8,
        max_events: 1024,
    };
    let reservation = SchedulerReservation {
        available_runtime_memory_bytes: prepared.plan.budget.memory_bytes,
        executor_overhead_limit_bytes: 256 * 1024 * 1024,
    };
    let start_started = Instant::now();
    let mut executor = DeterministicExecutor::start(
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
    let start_ns = start_started.elapsed().as_nanos() as u64;
    let allocation = executor.allocation();
    let resident_before = proc_status_bytes("VmRSS:");
    let cpu_before = process_cpu_ns();
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::SeqCst);
    let steady_started = Instant::now();
    let mut execution_error = None;
    let mut requested_stop_started = None;
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
            requested_stop_started = Some(Instant::now());
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
            execution_mode: "deterministic-single-lane",
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
            } else {
                "source-order; merge uses retained round-robin"
            },
            pressure: if matches!(args.workload, Workload::Overload) {
                args.pressure_policy.as_str()
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
                match args.termination_request {
                    TerminationRequest::Drain => "requested Drain while pressured",
                    TerminationRequest::Abort => "requested Abort while pressured",
                    TerminationRequest::Complete => unreachable!(),
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
        },
        exact_identity: ExactIdentity {
            logical_fixture: if matches!(args.workload, Workload::Overload) {
                format!(
                    "comparative-overload/{}/{}/{}/{}/{}/{}/{}/{}/{}",
                    args.pressure_policy.id(),
                    args.termination_request.as_str(),
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
        },
        latency: LatencyMeasurement {
            clock: "CLOCK_MONOTONIC via std::time::Instant",
            sample_stride: args.latency_sample_stride,
            samples_ns: prepared.observations.latencies.borrow().clone(),
        },
        semantic_notes: [
            "The public deterministic executor validates and preallocates the exact plan; timed execution does not disable contract checks.",
            if matches!(args.workload, Workload::Overload) {
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
            Workload::BoundedAsync | Workload::Overload | Workload::Fanout
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
        let raw = if args.identity_loop {
            run_identity_sample(&args, "warmup", trial, thermal_state)
        } else {
            run_sample(&args, "warmup", trial, thermal_state)
        };
        println!("{}", serde_json::to_string(&raw).unwrap());
    }
    for trial in 0..args.measured_trials {
        let raw = if args.identity_loop {
            run_identity_sample(&args, "measured", trial, "warmed")
        } else {
            run_sample(&args, "measured", trial, "warmed")
        };
        println!("{}", serde_json::to_string(&raw).unwrap());
    }
}
