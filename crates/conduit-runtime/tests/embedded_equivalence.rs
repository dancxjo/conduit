use std::cell::Cell;
use std::rc::Rc;

use conduit_core::{
    ArtifactDigest, AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee,
    Direction, ExecutionLimits, ExecutionPlan, ExecutionProfile, FlowCapacity, FlowEventKind,
    FlowPolicy, FlowQueueState, FlowWatermarks, Id, ImplementationMachine, InstancePath,
    InstantiationContext, LifecycleUsage, MemoryAccounting, MemoryCategory, MemoryClaim,
    PinnedDescriptor, PlanArtifact, PlanHostObservation, PlanResourceBudget, PlanValidationContext,
    Pressure, ReadyQueueDiscipline, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort,
    SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, SemanticHash, StopPolicy, TypeContractRef,
};
use conduit_embedded::{
    EmbeddedEventKind, EmbeddedHostServices, EmbeddedInterest, EmbeddedNode, EmbeddedOutcome,
    EmbeddedProfile, EmbeddedStep, EmbeddedStorage, EmbeddedValue, HostReply, InterestSet,
    RunControl, RunIdentity, RunStatus, STATIC_PLAN_SCHEMA_VERSION, StaticCord, StaticNode,
    StaticPlan, StepContext, execute_static_plan,
};
use conduit_rp2040_hil::PLAN_HASH as FIRMWARE_PLAN_HASH;
use conduit_runtime::{
    DeterministicExecutor, RuntimeValue, RuntimeValueEnvelope, ScheduledNode, SchedulerEventKind,
    SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep, SendStatus, StepIo,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/sample"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([9; 32]),
};
const CLAIMS: [MemoryClaim; 1] = [MemoryClaim {
    category: MemoryCategory::PortTransactions,
    accounting: MemoryAccounting::ExecutorAllocated,
    bytes: 320,
}];
const LIMITS: ExecutionLimits = ExecutionLimits {
    max_step_work: 4,
    max_retained_values: 0,
    max_retained_bytes: 0,
    max_scratch_bytes: 0,
    max_input_leases: 1,
    max_input_bytes: 8,
    max_output_reservations: 1,
    max_output_bytes: 8,
    max_transactions: 1,
    max_fragments_per_step: 0,
    max_pending_operations: 0,
    max_timers: 0,
    max_child_tasks: 0,
    max_host_buffer_bytes: 0,
    max_foreign_queue_items: 0,
    max_foreign_queue_bytes: 0,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 320,
    cancellation_ticks: 8,
};
const EQUIVALENCE_FIXTURE: &str = include_str!("../../../conformance/c5/embedded-equivalence.json");
const STATIC_NODES: [StaticNode<'static>; 3] = [
    StaticNode {
        semantic_path: Id("fixture/sensor"),
        implementation: Id("fixture/rp2040-sensor"),
        input_ports: 0,
        output_ports: 1,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
    StaticNode {
        semantic_path: Id("fixture/threshold"),
        implementation: Id("fixture/rp2040-threshold"),
        input_ports: 1,
        output_ports: 1,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
    StaticNode {
        semantic_path: Id("fixture/indicator"),
        implementation: Id("fixture/rp2040-indicator"),
        input_ports: 1,
        output_ports: 0,
        maximum_step_work: 2,
        nesting_depth: 1,
    },
];
const STATIC_CORDS: [StaticCord<'static>; 2] = [
    StaticCord {
        semantic_id: Id("fixture/sample"),
        producer_node: 0,
        producer_port: 0,
        consumer_node: 1,
        consumer_port: 0,
        slot_start: 0,
        capacity: 1,
        maximum_value_bytes: 4,
    },
    StaticCord {
        semantic_id: Id("fixture/decision"),
        producer_node: 1,
        producer_port: 0,
        consumer_node: 2,
        consumer_port: 0,
        slot_start: 1,
        capacity: 1,
        maximum_value_bytes: 1,
    },
];

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
    let mut profile = ExecutionProfile {
        id: Id("fixture/embedded-equivalence-profile"),
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
    profile.semantic_hash = profile.computed_semantic_hash(&mut [ZERO; 1]).unwrap();
    profile
}

fn with_equivalence_plans(
    test: impl FnOnce(ExecutionPlan<'_>, ExecutionPlan<'_>, &ExecutionProfile<'_>),
) {
    let profile = profile();
    let desktop_observations = [observation("fixture/desktop-report", "fixture/desktop", 1)];
    let rp2040_observations = [observation("fixture/rp2040-report", "fixture/rp2040", 2)];
    let desktop_artifacts = [
        PlanArtifact {
            id: Id("fixture/desktop-sensor-artifact"),
            digest: ArtifactDigest::from_bytes([3; 32]),
        },
        PlanArtifact {
            id: Id("fixture/desktop-threshold-artifact"),
            digest: ArtifactDigest::from_bytes([4; 32]),
        },
        PlanArtifact {
            id: Id("fixture/desktop-indicator-artifact"),
            digest: ArtifactDigest::from_bytes([5; 32]),
        },
    ];
    let rp2040_artifacts = [
        PlanArtifact {
            id: Id("fixture/rp2040-sensor-artifact"),
            digest: ArtifactDigest::from_bytes([6; 32]),
        },
        PlanArtifact {
            id: Id("fixture/rp2040-threshold-artifact"),
            digest: ArtifactDigest::from_bytes([7; 32]),
        },
        PlanArtifact {
            id: Id("fixture/rp2040-indicator-artifact"),
            digest: ArtifactDigest::from_bytes([8; 32]),
        },
    ];
    let instances = [
        InstancePath::new("fixture/sensor").unwrap(),
        InstancePath::new("fixture/threshold").unwrap(),
        InstancePath::new("fixture/indicator").unwrap(),
    ];
    let desktop_nodes = [
        node(
            instances[0],
            "fixture/sensor",
            20,
            "fixture/desktop-sensor",
            30,
            desktop_artifacts[0].id,
            desktop_observations[0].id,
            desktop_observations[0].host,
            &profile,
        ),
        node(
            instances[1],
            "fixture/threshold",
            21,
            "fixture/desktop-threshold",
            31,
            desktop_artifacts[1].id,
            desktop_observations[0].id,
            desktop_observations[0].host,
            &profile,
        ),
        node(
            instances[2],
            "fixture/indicator",
            22,
            "fixture/desktop-indicator",
            32,
            desktop_artifacts[2].id,
            desktop_observations[0].id,
            desktop_observations[0].host,
            &profile,
        ),
    ];
    let rp2040_nodes = [
        node(
            instances[0],
            "fixture/sensor",
            20,
            "fixture/rp2040-sensor",
            40,
            rp2040_artifacts[0].id,
            rp2040_observations[0].id,
            rp2040_observations[0].host,
            &profile,
        ),
        node(
            instances[1],
            "fixture/threshold",
            21,
            "fixture/rp2040-threshold",
            41,
            rp2040_artifacts[1].id,
            rp2040_observations[0].id,
            rp2040_observations[0].host,
            &profile,
        ),
        node(
            instances[2],
            "fixture/indicator",
            22,
            "fixture/rp2040-indicator",
            42,
            rp2040_artifacts[2].id,
            rp2040_observations[0].id,
            rp2040_observations[0].host,
            &profile,
        ),
    ];
    let capacity = FlowCapacity::new(1, 8, 8).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, 1, capacity).unwrap(),
    )
    .unwrap();
    let desktop_cords = [
        cord(
            "fixture/sample",
            desktop_nodes[0].instance,
            desktop_nodes[1].instance,
            50,
            flow,
        ),
        cord(
            "fixture/decision",
            desktop_nodes[1].instance,
            desktop_nodes[2].instance,
            51,
            flow,
        ),
    ];
    let rp2040_cords = [
        cord(
            "fixture/sample",
            rp2040_nodes[0].instance,
            rp2040_nodes[1].instance,
            50,
            flow,
        ),
        cord(
            "fixture/decision",
            rp2040_nodes[1].instance,
            rp2040_nodes[2].instance,
            51,
            flow,
        ),
    ];
    let mut desktop_plan = ExecutionPlan {
        schema_version: 0,
        identity: ZERO,
        source_semantic_hash: hash(60),
        resolver: pin("fixture/resolver", 61),
        resolver_policy_hash: hash(62),
        created_at: AuthorityTime {
            basis: Id("clock/monotonic"),
            tick: 1,
        },
        budget: PlanResourceBudget {
            memory_bytes: 32_000_000,
            storage_bytes: 0,
            cpu_units: 3,
            timers: 0,
            transports: 0,
            checkpoints: 0,
            evidence_bytes: 32_000_000,
        },
        host_observations: &desktop_observations,
        resources: &[],
        workloads: &[],
        artifacts: &desktop_artifacts,
        nodes: &desktop_nodes,
        cords: &desktop_cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &[],
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
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
    let mut rp2040_plan = ExecutionPlan {
        host_observations: &rp2040_observations,
        artifacts: &rp2040_artifacts,
        nodes: &rp2040_nodes,
        cords: &rp2040_cords,
        ..desktop_plan
    };
    desktop_plan.identity = desktop_plan.semantic_hash(&mut [ZERO; 32]).unwrap();
    rp2040_plan.identity = rp2040_plan.semantic_hash(&mut [ZERO; 32]).unwrap();
    assert_eq!(
        desktop_plan.source_semantic_hash,
        rp2040_plan.source_semantic_hash
    );
    assert_ne!(desktop_plan.identity, rp2040_plan.identity);
    assert_eq!(rp2040_plan.identity, FIRMWARE_PLAN_HASH);
    test(desktop_plan, rp2040_plan, &profile);
}

fn observation(id: &'static str, host: &'static str, byte: u8) -> PlanHostObservation<'static> {
    PlanHostObservation {
        id: Id(id),
        host: Id(host),
        semantic_hash: hash(byte),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 1_000,
    }
}

#[allow(clippy::too_many_arguments)]
fn node<'a>(
    instance: InstancePath<'a>,
    contract_id: &'static str,
    contract_byte: u8,
    implementation_id: &'static str,
    implementation_byte: u8,
    artifact: Id<'static>,
    host_observation: Id<'static>,
    host: Id<'static>,
    profile: &'a ExecutionProfile<'static>,
) -> ResolvedPlanNode<'a> {
    ResolvedPlanNode {
        instance,
        contract: pin(contract_id, contract_byte),
        implementation: pin(implementation_id, implementation_byte),
        lifecycle_policy: pin("fixture/lifecycle", 30),
        execution_profile: Some(profile),
        artifact,
        host_observation,
        host,
        allocation: PlanResourceBudget {
            memory_bytes: 512,
            cpu_units: 1,
            ..PlanResourceBudget::ZERO
        },
        required_resources: &[],
        required_effects: &[],
    }
}

fn cord<'a>(
    id: &'static str,
    from: InstancePath<'a>,
    to: InstancePath<'a>,
    byte: u8,
    flow: FlowPolicy<'a>,
) -> ResolvedPlanCord<'a> {
    ResolvedPlanCord {
        id: Id(id),
        from: ResolvedPlanPort {
            node: from,
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(byte),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: to,
            port: Id("in"),
            direction: Direction::Input,
            port_contract_hash: hash(byte + 20),
            value_type: TYPE,
        },
        flow,
        queue_memory_bytes: 8,
    }
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
enum DesktopDriver {
    Sensor { emitted: bool },
    Threshold,
    Indicator { value: Rc<Cell<Option<u64>>> },
}

impl SchedulerNode for DesktopDriver {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                match io
                    .send(
                        0,
                        RuntimeValue {
                            handle: 42,
                            accounted_bytes: 4,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        },
                        None,
                    )
                    .unwrap()
                {
                    SendStatus::Reserved => {
                        *emitted = true;
                        SchedulerStep::Progress
                    }
                    SendStatus::WouldBlock => {
                        io.wait_for_output(0).unwrap();
                        SchedulerStep::Pending
                    }
                    _ => SchedulerStep::Failed {
                        code: Id("fixture/send"),
                    },
                }
            }
            Self::Threshold => {
                if let Some(sample) = io.receive(0).unwrap() {
                    assert_eq!(sample.handle, 42);
                    match io
                        .send(
                            1,
                            RuntimeValue {
                                handle: u64::from(sample.handle >= 40),
                                accounted_bytes: 1,
                                envelope: RuntimeValueEnvelope::EMPTY,
                            },
                            None,
                        )
                        .unwrap()
                    {
                        SendStatus::Reserved => SchedulerStep::Progress,
                        SendStatus::WouldBlock => {
                            io.wait_for_output(1).unwrap();
                            SchedulerStep::Pending
                        }
                        _ => SchedulerStep::Failed {
                            code: Id("fixture/threshold"),
                        },
                    }
                } else if matches!(io.input_state(0).unwrap(), FlowQueueState::Completed) {
                    SchedulerStep::Completed
                } else {
                    io.wait_for_input(0).unwrap();
                    SchedulerStep::Pending
                }
            }
            Self::Indicator { value } => {
                if let Some(decision) = io.receive(1).unwrap() {
                    value.set(Some(decision.handle));
                    SchedulerStep::Progress
                } else if matches!(io.input_state(1).unwrap(), FlowQueueState::Completed) {
                    SchedulerStep::Completed
                } else {
                    io.wait_for_input(1).unwrap();
                    SchedulerStep::Pending
                }
            }
        }
    }
}

fn desktop_executor<'a>(
    plan: &'a ExecutionPlan<'a>,
    profile: &'a ExecutionProfile<'a>,
    indicator: Rc<Cell<Option<u64>>>,
) -> DeterministicExecutor<DesktopDriver> {
    DeterministicExecutor::start(
        plan,
        PlanValidationContext {
            supported_schema_version: 0,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        },
        SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 64,
            max_tick: 128,
            max_consecutive_yields: 4,
            max_events: 128,
        },
        SchedulerReservation {
            available_runtime_memory_bytes: 32_000_000,
            executor_overhead_limit_bytes: 31_000_000,
        },
        vec![
            ScheduledNode {
                driver: DesktopDriver::Sensor { emitted: false },
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: DesktopDriver::Threshold,
                machine: machine(profile, &plan.nodes[1]),
            },
            ScheduledNode {
                driver: DesktopDriver::Indicator { value: indicator },
                machine: machine(profile, &plan.nodes[2]),
            },
        ],
    )
    .unwrap()
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    prepared: usize,
    accepted: Vec<u64>,
    consumed: Vec<u64>,
    pressure_entered: usize,
    pressure_cleared: usize,
    completed: usize,
    succeeded: bool,
}

fn normalize_desktop(executor: &DeterministicExecutor<DesktopDriver>) -> Normalized {
    let mut normalized = Normalized {
        prepared: 0,
        accepted: Vec::new(),
        consumed: Vec::new(),
        pressure_entered: 0,
        pressure_cleared: 0,
        completed: 0,
        succeeded: false,
    };
    for event in executor.events() {
        match event.kind {
            SchedulerEventKind::NodePrepared => normalized.prepared += 1,
            SchedulerEventKind::ValueAccepted => {
                normalized.accepted.push(event.value_handle.unwrap())
            }
            SchedulerEventKind::ValueConsumed => {
                normalized.consumed.push(event.value_handle.unwrap())
            }
            SchedulerEventKind::Cord(FlowEventKind::PressureEntered) => {
                normalized.pressure_entered += 1
            }
            SchedulerEventKind::Cord(FlowEventKind::PressureCleared) => {
                normalized.pressure_cleared += 1
            }
            SchedulerEventKind::NodeOutcome {
                outcome: conduit_core::StepOutcomeKind::Completed,
            } => normalized.completed += 1,
            SchedulerEventKind::Terminal(conduit_core::TerminalClass::Succeeded) => {
                normalized.succeeded = true
            }
            _ => {}
        }
    }
    normalized
}

struct EmbeddedHost {
    indicator: Option<u64>,
}

impl EmbeddedHostServices<16> for EmbeddedHost {
    fn invoke(&mut self, binding: u16, request: EmbeddedValue<16>) -> HostReply<16> {
        match binding {
            0 => HostReply::Completed(EmbeddedValue::from_slice(&42_u32.to_be_bytes()).unwrap()),
            1 => {
                self.indicator = Some(u64::from(request.bytes[0]));
                HostReply::Completed(EmbeddedValue::EMPTY)
            }
            _ => HostReply::Failed(Id("fixture/host")),
        }
    }
}

enum EmbeddedDriver {
    Sensor { emitted: bool },
    Threshold,
    Indicator,
}

impl EmbeddedNode<EmbeddedHost, 16, 4, 4> for EmbeddedDriver {
    fn step(&mut self, context: &mut StepContext<'_, EmbeddedHost, 16, 4>) -> EmbeddedStep<4> {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return EmbeddedStep::completed();
                }
                let HostReply::Completed(sample) =
                    context.invoke_host(0, EmbeddedValue::EMPTY).unwrap()
                else {
                    return failed();
                };
                context.send(0, sample).unwrap();
                *emitted = true;
                EmbeddedStep::progress()
            }
            Self::Threshold => {
                if let Some(sample) = context.input(0) {
                    let sample = u32::from_be_bytes(sample.bytes[..4].try_into().unwrap());
                    context.consume(0).unwrap();
                    context
                        .send(
                            0,
                            EmbeddedValue::from_slice(&[u8::from(sample >= 40)]).unwrap(),
                        )
                        .unwrap();
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
            Self::Indicator => {
                if context.input(0).is_some() {
                    let value = context.consume(0).unwrap();
                    let _ = context.invoke_host(1, value).unwrap();
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
        }
    }
}

fn failed() -> EmbeddedStep<4> {
    EmbeddedStep {
        outcome: EmbeddedOutcome::Failed(Id("fixture/embedded")),
        interests: InterestSet::EMPTY,
    }
}

fn embedded_profile() -> EmbeddedProfile {
    let mut profile = EmbeddedProfile {
        identity: ZERO,
        maximum_nodes: 3,
        maximum_cords: 2,
        maximum_ports: 4,
        maximum_queue_slots: 2,
        maximum_value_bytes: 16,
        maximum_evidence_records: 64,
        maximum_timers: 2,
        maximum_interests_per_node: 4,
        maximum_nesting: 1,
        maximum_timer_delay: 64,
        static_ram_budget_bytes: 64 * 1024,
        stack_budget_bytes: 4 * 1024,
        flash_budget_bytes: 64 * 1024,
    };
    profile.seal().unwrap();
    profile
}

fn normalize_embedded(events: &[conduit_embedded::EmbeddedEvent<16>]) -> Normalized {
    let mut normalized = Normalized {
        prepared: 0,
        accepted: Vec::new(),
        consumed: Vec::new(),
        pressure_entered: 0,
        pressure_cleared: 0,
        completed: 0,
        succeeded: false,
    };
    for event in events {
        match event.kind {
            EmbeddedEventKind::NodePrepared => normalized.prepared += 1,
            EmbeddedEventKind::ValueAccepted => {
                normalized.accepted.push(decode_value(event.value.unwrap()))
            }
            EmbeddedEventKind::ValueConsumed => {
                normalized.consumed.push(decode_value(event.value.unwrap()))
            }
            EmbeddedEventKind::PressureEntered => normalized.pressure_entered += 1,
            EmbeddedEventKind::PressureCleared => normalized.pressure_cleared += 1,
            EmbeddedEventKind::NodeCompleted => normalized.completed += 1,
            EmbeddedEventKind::RunSucceeded => normalized.succeeded = true,
            _ => {}
        }
    }
    normalized
}

fn decode_value(value: EmbeddedValue<16>) -> u64 {
    match value.length {
        1 => u64::from(value.bytes[0]),
        4 => u64::from(u32::from_be_bytes(value.bytes[..4].try_into().unwrap())),
        other => panic!("unexpected value width {other}"),
    }
}

fn equivalence_fixture_expected(id: &str) -> serde_json::Value {
    let fixture: serde_json::Value = serde_json::from_str(EQUIVALENCE_FIXTURE).unwrap();
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing executed equivalence fixture case {id}"))["expected"]
        .clone()
}

#[test]
fn desktop_and_rp2040_execute_one_semantic_plan_with_normalized_equivalence() {
    let expected = equivalence_fixture_expected("same-plan-normalized-equivalence");
    with_equivalence_plans(|desktop_plan, rp2040_plan, profile| {
        let desktop_indicator = Rc::new(Cell::new(None));
        let mut desktop = desktop_executor(&desktop_plan, profile, desktop_indicator.clone());
        assert_eq!(
            desktop.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(desktop_indicator.get(), Some(1));

        let embedded_profile = embedded_profile();
        let static_plan = StaticPlan {
            schema_version: STATIC_PLAN_SCHEMA_VERSION,
            full_plan_hash: rp2040_plan.identity,
            profile_hash: embedded_profile.identity,
            nodes: &STATIC_NODES,
            cords: &STATIC_CORDS,
        };
        let mut storage = EmbeddedStorage::<3, 2, 4, 2, 16, 64, 2, 4>::new();
        let mut embedded_host = EmbeddedHost { indicator: None };
        let summary = execute_static_plan(
            &static_plan,
            &embedded_profile,
            &mut storage,
            &mut [
                EmbeddedDriver::Sensor { emitted: false },
                EmbeddedDriver::Threshold,
                EmbeddedDriver::Indicator,
            ],
            &mut embedded_host,
            RunIdentity {
                boot_id: [5; 16],
                run_sequence: 1,
            },
            RunControl {
                maximum_decisions: 64,
                cancellation_at_decision: None,
                initial_tick: 0,
            },
        )
        .unwrap();
        assert_eq!(summary.status, RunStatus::Succeeded);
        assert_eq!(embedded_host.indicator, Some(1));
        let normalized_equal = normalize_desktop(&desktop) == normalize_embedded(storage.events());
        assert!(normalized_equal);
        assert!(
            storage
                .events()
                .iter()
                .all(|event| event.plan == rp2040_plan.identity)
        );
        assert_eq!(
            serde_json::json!({
                "same_source_semantic_hash": desktop_plan.source_semantic_hash == rp2040_plan.source_semantic_hash,
                "distinct_exact_execution_plan_hashes": desktop_plan.identity != rp2040_plan.identity,
                "firmware_bound_to_rp2040_plan_hash": rp2040_plan.identity == FIRMWARE_PLAN_HASH,
                "normalized_lifecycle_values_pressure_terminal_equal": normalized_equal
            }),
            expected
        );
    });
}

#[test]
fn desktop_and_rp2040_abort_cancellation_are_both_terminal() {
    let expected = equivalence_fixture_expected("same-plan-abort-cancellation-equivalence");
    with_equivalence_plans(|desktop_plan, rp2040_plan, profile| {
        let mut desktop = desktop_executor(&desktop_plan, profile, Rc::new(Cell::new(None)));
        desktop.cancel(StopPolicy::Abort).unwrap();
        assert_eq!(
            desktop.run_until_stalled().unwrap(),
            SchedulerStatus::Cancelled
        );
        assert!(desktop.events().any(|event| matches!(
            event.kind,
            SchedulerEventKind::CancellationRequested {
                stop: StopPolicy::Abort
            }
        )));
        assert!(desktop.events().any(|event| matches!(
            event.kind,
            SchedulerEventKind::Terminal(conduit_core::TerminalClass::Cancelled)
        )));

        let embedded_profile = embedded_profile();
        let static_plan = StaticPlan {
            schema_version: STATIC_PLAN_SCHEMA_VERSION,
            full_plan_hash: rp2040_plan.identity,
            profile_hash: embedded_profile.identity,
            nodes: &STATIC_NODES,
            cords: &STATIC_CORDS,
        };
        let mut storage = EmbeddedStorage::<3, 2, 4, 2, 16, 64, 2, 4>::new();
        let summary = execute_static_plan(
            &static_plan,
            &embedded_profile,
            &mut storage,
            &mut [
                EmbeddedDriver::Sensor { emitted: false },
                EmbeddedDriver::Threshold,
                EmbeddedDriver::Indicator,
            ],
            &mut EmbeddedHost { indicator: None },
            RunIdentity {
                boot_id: [5; 16],
                run_sequence: 2,
            },
            RunControl {
                maximum_decisions: 4,
                cancellation_at_decision: Some(0),
                initial_tick: 0,
            },
        )
        .unwrap();
        assert_eq!(summary.status, RunStatus::Cancelled);
        assert!(
            storage
                .events()
                .iter()
                .any(|event| { event.kind == EmbeddedEventKind::CancellationRequested })
        );
        assert!(
            storage
                .events()
                .iter()
                .any(|event| event.kind == EmbeddedEventKind::RunCancelled)
        );
        assert_eq!(
            serde_json::json!({
                "same_source_semantic_hash": desktop_plan.source_semantic_hash == rp2040_plan.source_semantic_hash,
                "distinct_exact_execution_plan_hashes": desktop_plan.identity != rp2040_plan.identity,
                "firmware_bound_to_rp2040_plan_hash": rp2040_plan.identity == FIRMWARE_PLAN_HASH,
                "desktop_status": "cancelled",
                "embedded_status": "cancelled"
            }),
            expected
        );
    });
}
