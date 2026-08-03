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
    Direction, ExecutionLimits, ExecutionPlan, ExecutionProfile, FlowCapacity, FlowPolicy,
    FlowQueueState, FlowWatermarks, Id, ImplementationMachine, InstancePath, InstantiationContext,
    LifecycleUsage, MemoryAccounting, MemoryCategory, MemoryClaim, PinnedDescriptor, PlanArtifact,
    PlanHostObservation, PlanResourceBudget, PlanValidationContext, Pressure, ReadyQueueDiscipline,
    ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort, SCHEDULER_CONTRACT_VERSION,
    SchedulerPolicy, SemanticHash, TypeContractRef,
};
use conduit_runtime::{
    DeterministicExecutor, RuntimeValue, RuntimeValueEnvelope, ScheduledNode, SchedulerNode,
    SchedulerReservation, SchedulerStatus, SchedulerStep, SendStatus, StepIo,
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
    bytes: 256,
}];
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
    implementation_memory_bytes: 256,
    cancellation_ticks: 8,
};

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Workload {
    Map,
    MapFilter,
    Merge,
    BoundedAsync,
}

impl Workload {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::MapFilter => "map-filter",
            Self::Merge => "merge",
            Self::BoundedAsync => "bounded-async",
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
}

#[derive(Serialize)]
struct PhaseTimes {
    assembly_ns: u64,
    plan_seal_ns: Option<u64>,
    start_ns: Option<u64>,
    steady_ns: u64,
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
}

#[derive(Serialize)]
struct ExactIdentity {
    logical_fixture: String,
    plan_identity: Option<String>,
    source_semantic_hash: Option<String>,
    artifact_digest: Option<String>,
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
    process_cpu_ns: Option<u64>,
    accepted_values: u64,
    useful_outputs: u64,
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
    stride: u64,
}

enum BenchNode {
    Source {
        next: u64,
        end: u64,
        cord: usize,
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
    },
}

impl SchedulerNode for BenchNode {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match self {
            Self::Source {
                next,
                end,
                cord,
                observations,
            } => {
                if *next == *end {
                    return SchedulerStep::Completed;
                }
                let index = *next;
                if index % observations.stride == 0 {
                    let sample = usize::try_from(index / observations.stride).unwrap();
                    observations.starts.borrow_mut()[sample] = Some(Instant::now());
                }
                let value = RuntimeValue {
                    handle: index,
                    accounted_bytes: 8,
                    envelope: RuntimeValueEnvelope::EMPTY,
                };
                match io.send(*cord, value, None) {
                    Ok(SendStatus::Reserved) => {
                        *next += 1;
                        observations
                            .accepted_values
                            .set(observations.accepted_values.get() + 1);
                        SchedulerStep::Progress
                    }
                    Ok(SendStatus::WouldBlock) => {
                        io.wait_for_output(*cord).unwrap();
                        SchedulerStep::Pending
                    }
                    _ => SchedulerStep::Failed {
                        code: Id("benchmark/source-output-rejected"),
                    },
                }
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
            } => match io.receive(*input) {
                Ok(Some(value)) => {
                    observations
                        .useful_outputs
                        .set(observations.useful_outputs.get() + 1);
                    if value.handle % observations.stride == 0 {
                        let sample = usize::try_from(value.handle / observations.stride).unwrap();
                        if let Some(start) = observations.starts.borrow()[sample] {
                            observations
                                .latencies
                                .borrow_mut()
                                .push(start.elapsed().as_nanos() as u64);
                        }
                    }
                    SchedulerStep::Progress
                }
                _ if matches!(io.input_state(*input), Ok(FlowQueueState::Completed)) => {
                    SchedulerStep::Completed
                }
                _ => {
                    io.wait_for_input(*input).unwrap();
                    SchedulerStep::Pending
                }
            },
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

fn profile() -> ExecutionProfile<'static> {
    let mut value = ExecutionProfile {
        id: Id("benchmark/reference-profile"),
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
    value.semantic_hash = value.computed_semantic_hash(&mut [ZERO; 1]).unwrap();
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
            caller_memory_bytes: 256,
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
    assert!(
        args.latency_sample_stride > 0,
        "sample stride must be positive"
    );
    assert!(
        !matches!(args.workload, Workload::BoundedAsync),
        "the single-lane reference executor cannot claim an asynchronous boundary"
    );

    let assembly_started = Instant::now();
    let profile = Box::leak(Box::new(profile()));
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
    let observations = Observations {
        values: Rc::new(RefCell::new((0..args.values).collect())),
        starts: Rc::new(RefCell::new(vec![None; sample_count])),
        latencies: Rc::new(RefCell::new(Vec::with_capacity(sample_count))),
        accepted_values: Rc::new(Cell::new(0)),
        useful_outputs: Rc::new(Cell::new(0)),
        stride: args.latency_sample_stride,
    };
    assert_eq!(observations.values.borrow().len(), value_count);

    let transform_count = match args.workload {
        Workload::Merge => args.operators.saturating_sub(1),
        _ => args.operators,
    };
    let mut node_roles = if matches!(args.workload, Workload::Merge) {
        vec!["source", "source", "merge"]
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
    node_roles.push("sink");
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
                memory_bytes: 512,
                cpu_units: 1,
                ..PlanResourceBudget::ZERO
            },
            required_resources: &[],
            required_effects: &[],
        });
    }

    let capacity =
        FlowCapacity::new(args.queue_items, 16, u64::from(args.queue_items) * 16).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, args.queue_items, capacity).unwrap(),
    )
    .unwrap();
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
        });
        drivers.push(BenchNode::Source {
            next: split,
            end: args.values,
            cord: 1,
            observations: observations.clone(),
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
        });
    } else {
        drivers.push(BenchNode::Source {
            next: 0,
            end: args.values,
            cord: 0,
            observations: observations.clone(),
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
        });
    }

    let assembly_ns = assembly_started.elapsed().as_nanos() as u64;
    let artifacts = Box::leak(artifacts.into_boxed_slice());
    let nodes = Box::leak(nodes.into_boxed_slice());
    let cords = Box::leak(cords.into_boxed_slice());
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
            },
            &args.operators.to_string(),
            &args.values.to_string(),
            &args.queue_items.to_string(),
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
    let seal_started = Instant::now();
    let scratch_count = plan.validation_scratch_count().unwrap().max(1);
    plan.identity = plan.semantic_hash(&mut vec![ZERO; scratch_count]).unwrap();
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
    let policy = SchedulerPolicy {
        schema_version: SCHEDULER_CONTRACT_VERSION,
        ready_queue: ReadyQueueDiscipline::RoundRobin,
        max_decisions: args
            .values
            .saturating_mul(u64::try_from(args.operators + 8).unwrap()),
        max_tick: args
            .values
            .saturating_mul(u64::try_from(args.operators + 9).unwrap()),
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
    let status = loop {
        let status = executor.run_one().unwrap();
        if executor.event_count() >= 768 {
            executor
                .acknowledge_events_through(executor.next_event_cursor())
                .unwrap();
        }
        if !matches!(status, SchedulerStatus::Running) {
            break status;
        }
    };
    let steady_ns = steady_started.elapsed().as_nanos() as u64;
    MEASURING.store(false, Ordering::SeqCst);
    let cpu_ns = process_cpu_ns()
        .zip(cpu_before)
        .map(|(after, before)| after - before);
    assert_eq!(status, SchedulerStatus::Succeeded);
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
            ordering: "source-order; merge uses retained round-robin",
            pressure: "bounded FIFO block",
            terminal: "complete after all accepted values drain",
            loss: "none",
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
        },
        process_cpu_ns: cpu_ns,
        accepted_values: prepared.observations.accepted_values.get(),
        useful_outputs: prepared.observations.useful_outputs.get(),
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
            "The handle-backed u64 fixture isolates scheduler cost and is not the optimized hosted streaming mode, which is not yet available.",
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
        !matches!(args.workload, Workload::BoundedAsync),
        "an identity loop cannot model a bounded asynchronous boundary"
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
        },
        process_cpu_ns: cpu_ns,
        accepted_values,
        useful_outputs,
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
