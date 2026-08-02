use conduit_core::{
    CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, ExecutorKind, Id, PinnedDescriptor,
    PlanResourceBudget, ReportCapability, ReportResource, ResourceRef, SemanticHash,
    validate_capability_report,
};
use conduit_embedded::{
    EmbeddedError, EmbeddedEvent, EmbeddedEventKind, EmbeddedHostServices, EmbeddedInterest,
    EmbeddedNode, EmbeddedOutcome, EmbeddedProfile, EmbeddedStep, EmbeddedStorage, EmbeddedSubject,
    EmbeddedValue, FirmwareReplacementLevel, HIL_PROTOCOL_VERSION, HilEventFrame, HilRequest,
    HilRunHeader, HilRunStatus, HostReply, InterestSet, MAXIMUM_CORDS, MAXIMUM_EVIDENCE_RECORDS,
    MAXIMUM_INTERESTS_PER_NODE, MAXIMUM_NESTING, MAXIMUM_NODES, MAXIMUM_PORTS, MAXIMUM_QUEUE_SLOTS,
    MAXIMUM_TIMER_DELAY, MAXIMUM_TIMERS, MAXIMUM_VALUE_BYTES, RP2040_SRAM_BYTES, RunControl,
    RunIdentity, RunStatus, STATIC_PLAN_SCHEMA_VERSION, StaticCord, StaticNode, StaticPlan,
    StepContext, StorageShape, deadline_reached, execute_static_plan,
    validate_firmware_replacement, validate_static_plan,
};

const FIXTURE: &str = include_str!("../../../conformance/c5/embedded-rp2040.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const PLAN_HASH: SemanticHash = SemanticHash::from_bytes([7; 32]);

const NODES: [StaticNode<'static>; 3] = [
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
const CORDS: [StaticCord<'static>; 2] = [
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
const MAX_NODE_IDS: [Id<'static>; 32] = [
    Id("fixture/max-node-00"),
    Id("fixture/max-node-01"),
    Id("fixture/max-node-02"),
    Id("fixture/max-node-03"),
    Id("fixture/max-node-04"),
    Id("fixture/max-node-05"),
    Id("fixture/max-node-06"),
    Id("fixture/max-node-07"),
    Id("fixture/max-node-08"),
    Id("fixture/max-node-09"),
    Id("fixture/max-node-10"),
    Id("fixture/max-node-11"),
    Id("fixture/max-node-12"),
    Id("fixture/max-node-13"),
    Id("fixture/max-node-14"),
    Id("fixture/max-node-15"),
    Id("fixture/max-node-16"),
    Id("fixture/max-node-17"),
    Id("fixture/max-node-18"),
    Id("fixture/max-node-19"),
    Id("fixture/max-node-20"),
    Id("fixture/max-node-21"),
    Id("fixture/max-node-22"),
    Id("fixture/max-node-23"),
    Id("fixture/max-node-24"),
    Id("fixture/max-node-25"),
    Id("fixture/max-node-26"),
    Id("fixture/max-node-27"),
    Id("fixture/max-node-28"),
    Id("fixture/max-node-29"),
    Id("fixture/max-node-30"),
    Id("fixture/max-node-31"),
];
const MAX_CORD_IDS: [Id<'static>; 48] = [
    Id("fixture/max-cord-00"),
    Id("fixture/max-cord-01"),
    Id("fixture/max-cord-02"),
    Id("fixture/max-cord-03"),
    Id("fixture/max-cord-04"),
    Id("fixture/max-cord-05"),
    Id("fixture/max-cord-06"),
    Id("fixture/max-cord-07"),
    Id("fixture/max-cord-08"),
    Id("fixture/max-cord-09"),
    Id("fixture/max-cord-10"),
    Id("fixture/max-cord-11"),
    Id("fixture/max-cord-12"),
    Id("fixture/max-cord-13"),
    Id("fixture/max-cord-14"),
    Id("fixture/max-cord-15"),
    Id("fixture/max-cord-16"),
    Id("fixture/max-cord-17"),
    Id("fixture/max-cord-18"),
    Id("fixture/max-cord-19"),
    Id("fixture/max-cord-20"),
    Id("fixture/max-cord-21"),
    Id("fixture/max-cord-22"),
    Id("fixture/max-cord-23"),
    Id("fixture/max-cord-24"),
    Id("fixture/max-cord-25"),
    Id("fixture/max-cord-26"),
    Id("fixture/max-cord-27"),
    Id("fixture/max-cord-28"),
    Id("fixture/max-cord-29"),
    Id("fixture/max-cord-30"),
    Id("fixture/max-cord-31"),
    Id("fixture/max-cord-32"),
    Id("fixture/max-cord-33"),
    Id("fixture/max-cord-34"),
    Id("fixture/max-cord-35"),
    Id("fixture/max-cord-36"),
    Id("fixture/max-cord-37"),
    Id("fixture/max-cord-38"),
    Id("fixture/max-cord-39"),
    Id("fixture/max-cord-40"),
    Id("fixture/max-cord-41"),
    Id("fixture/max-cord-42"),
    Id("fixture/max-cord-43"),
    Id("fixture/max-cord-44"),
    Id("fixture/max-cord-45"),
    Id("fixture/max-cord-46"),
    Id("fixture/max-cord-47"),
];

type Storage = EmbeddedStorage<3, 2, 4, 2, 16, 64, 4, 4>;

fn profile() -> EmbeddedProfile {
    let mut profile = EmbeddedProfile {
        identity: ZERO,
        maximum_nodes: 3,
        maximum_cords: 2,
        maximum_ports: 4,
        maximum_queue_slots: 2,
        maximum_value_bytes: 16,
        maximum_evidence_records: 64,
        maximum_timers: 4,
        maximum_interests_per_node: 4,
        maximum_nesting: 2,
        maximum_timer_delay: 1_000,
        static_ram_budget_bytes: 64 * 1024,
        stack_budget_bytes: 4 * 1024,
        flash_budget_bytes: 64 * 1024,
    };
    profile.seal().unwrap();
    profile
}

fn maximum_profile() -> EmbeddedProfile {
    let mut profile = EmbeddedProfile {
        identity: ZERO,
        maximum_nodes: MAXIMUM_NODES,
        maximum_cords: MAXIMUM_CORDS,
        maximum_ports: MAXIMUM_PORTS,
        maximum_queue_slots: MAXIMUM_QUEUE_SLOTS,
        maximum_value_bytes: MAXIMUM_VALUE_BYTES,
        maximum_evidence_records: MAXIMUM_EVIDENCE_RECORDS,
        maximum_timers: MAXIMUM_TIMERS,
        maximum_interests_per_node: MAXIMUM_INTERESTS_PER_NODE,
        maximum_nesting: MAXIMUM_NESTING,
        maximum_timer_delay: MAXIMUM_TIMER_DELAY,
        static_ram_budget_bytes: RP2040_SRAM_BYTES,
        stack_budget_bytes: 8 * 1024,
        flash_budget_bytes: 64 * 1024,
    };
    profile.seal().unwrap();
    profile
}

fn plan<'a>(
    profile: &EmbeddedProfile,
    nodes: &'a [StaticNode<'a>],
    cords: &'a [StaticCord<'a>],
) -> StaticPlan<'a> {
    StaticPlan {
        schema_version: STATIC_PLAN_SCHEMA_VERSION,
        full_plan_hash: PLAN_HASH,
        profile_hash: profile.identity,
        nodes,
        cords,
    }
}

#[derive(Default)]
struct Host {
    indicator: bool,
    cancelled: u8,
}

impl EmbeddedHostServices<16> for Host {
    fn invoke(&mut self, binding: u16, request: EmbeddedValue<16>) -> HostReply<16> {
        match binding {
            0 => HostReply::Completed(EmbeddedValue::from_slice(&42_u32.to_be_bytes()).unwrap()),
            1 if request.length == 1 => {
                self.indicator = request.bytes[0] != 0;
                HostReply::Completed(EmbeddedValue::EMPTY)
            }
            _ => HostReply::Failed(Id("fixture/host-failed")),
        }
    }
}

#[derive(Clone, Copy)]
enum Driver {
    Sensor { emitted: bool },
    Threshold,
    Indicator,
}

fn drivers() -> [Driver; 3] {
    [
        Driver::Sensor { emitted: false },
        Driver::Threshold,
        Driver::Indicator,
    ]
}

impl EmbeddedNode<Host, 16, 4, 4> for Driver {
    fn step(&mut self, context: &mut StepContext<'_, Host, 16, 4>) -> EmbeddedStep<4> {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return EmbeddedStep::completed();
                }
                let HostReply::Completed(sample) =
                    context.invoke_host(0, EmbeddedValue::EMPTY).unwrap()
                else {
                    return EmbeddedStep {
                        outcome: EmbeddedOutcome::Failed(Id("fixture/sample")),
                        interests: InterestSet::EMPTY,
                    };
                };
                context.send(0, sample).unwrap();
                *emitted = true;
                EmbeddedStep::progress()
            }
            Self::Threshold => {
                if let Some(sample) = context.input(0) {
                    let sample = u32::from_be_bytes([
                        sample.bytes[0],
                        sample.bytes[1],
                        sample.bytes[2],
                        sample.bytes[3],
                    ]);
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
                    let decision = context.consume(0).unwrap();
                    let reply = context.invoke_host(1, decision).unwrap();
                    if matches!(reply, HostReply::Failed(_)) {
                        return EmbeddedStep {
                            outcome: EmbeddedOutcome::Failed(Id("fixture/indicator")),
                            interests: InterestSet::EMPTY,
                        };
                    }
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
        }
    }

    fn cancel(&mut self, host: &mut Host) {
        host.cancelled += 1;
    }
}

fn run_representative(
    profile: &EmbeddedProfile,
    storage: &mut Storage,
    host: &mut Host,
    identity: RunIdentity,
    control: RunControl,
) -> Result<conduit_embedded::RunSummary, EmbeddedError> {
    execute_static_plan(
        &plan(profile, &NODES, &CORDS),
        profile,
        storage,
        &mut drivers(),
        host,
        identity,
        control,
    )
}

fn standard_control() -> RunControl {
    RunControl {
        maximum_decisions: 64,
        cancellation_at_decision: None,
        initial_tick: 0,
    }
}

fn preflight_case(id: &str) -> serde_json::Value {
    let mut selected = profile();
    let mut nodes = NODES;
    let mut cords = CORDS;
    let mut shape = StorageShape::of::<3, 2, 4, 2, 16, 64, 4, 4>();
    match id {
        "exact-storage-shape" => {}
        "profile-identity-mutation" => selected.maximum_nodes -= 1,
        "node-maximum-rejected-before-start" => {
            selected.maximum_nodes = 2;
            selected.seal().unwrap();
        }
        "cord-maximum-rejected-before-start" => {
            selected.maximum_cords = 1;
            selected.seal().unwrap();
        }
        "port-maximum-rejected-before-start" => {
            selected.maximum_ports = 3;
            selected.seal().unwrap();
        }
        "queue-maximum-rejected-before-start" => {
            selected.maximum_queue_slots = 1;
            selected.seal().unwrap();
        }
        "value-maximum-rejected-before-start" => {
            cords[0].maximum_value_bytes = 17;
        }
        "caller-static-storage-rejected-before-start" => {
            shape.static_bytes = selected.static_ram_budget_bytes + 1;
        }
        "overlapping-queue-layout" => cords[1].slot_start = 0,
        "duplicate-node-semantic-path" => nodes[1].semantic_path = nodes[0].semantic_path,
        "duplicate-cord-semantic-id" => cords[1].semantic_id = cords[0].semantic_id,
        "unsupported-fanout-layout" => {
            cords[1].producer_node = 0;
            cords[1].producer_port = 0;
        }
        "nesting-maximum-rejected-before-start" => nodes[0].nesting_depth = 3,
        "maximum-supported-graph-preflights" => return maximum_graph_case(),
        other => panic!("unimplemented embedded preflight vector `{other}`"),
    }
    match validate_static_plan(&plan(&selected, &nodes, &cords), &selected, shape) {
        Ok(_) => serde_json::json!({"accepted": true}),
        Err(error) => serde_json::json!({"accepted": false, "code": error.code()}),
    }
}

fn executor_case(id: &str) -> serde_json::Value {
    match id {
        "representative-sensor-threshold-indicator" => {
            let mut storage = Storage::new();
            let mut host = Host::default();
            let summary = run_representative(
                &profile(),
                &mut storage,
                &mut host,
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                standard_control(),
            )
            .unwrap();
            serde_json::json!({
                "accepted": true,
                "status": if summary.status == RunStatus::Succeeded {"succeeded"} else {"other"},
                "indicator": host.indicator
            })
        }
        "decision-ceiling-is-terminal" => {
            let error = run_representative(
                &profile(),
                &mut Storage::new(),
                &mut Host::default(),
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                RunControl {
                    maximum_decisions: 1,
                    ..standard_control()
                },
            )
            .unwrap_err();
            serde_json::json!({"accepted": false, "code": error.code()})
        }
        "evidence-ceiling-is-terminal" => {
            let mut selected = profile();
            selected.maximum_evidence_records = 4;
            selected.seal().unwrap();
            let error = run_representative(
                &selected,
                &mut Storage::new(),
                &mut Host::default(),
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                standard_control(),
            )
            .unwrap_err();
            serde_json::json!({"accepted": false, "code": error.code()})
        }
        "queue-pressure-and-clear-are-evidenced" => {
            let mut storage = Storage::new();
            run_representative(
                &profile(),
                &mut storage,
                &mut Host::default(),
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                standard_control(),
            )
            .unwrap();
            serde_json::json!({
                "pressure_entered": storage.events().iter().any(|event| event.kind == EmbeddedEventKind::PressureEntered),
                "pressure_cleared": storage.events().iter().any(|event| event.kind == EmbeddedEventKind::PressureCleared)
            })
        }
        "timer-wraparound-wakes-exactly" => timer_case(1),
        "ambiguous-timer-wraparound-is-rejected" => timer_case(0x7fff_ffff),
        "abort-cancellation-is-bounded" => {
            let mut storage = Storage::new();
            let mut host = Host::default();
            let summary = run_representative(
                &profile(),
                &mut storage,
                &mut host,
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                RunControl {
                    cancellation_at_decision: Some(2),
                    ..standard_control()
                },
            )
            .unwrap();
            assert_eq!(host.cancelled, 3);
            serde_json::json!({"accepted": true, "status": if summary.status == RunStatus::Cancelled {"cancelled"} else {"other"}})
        }
        "reboot-changes-session-attribution" => {
            let selected = profile();
            let mut first_storage = Storage::new();
            let mut second_storage = Storage::new();
            run_representative(
                &selected,
                &mut first_storage,
                &mut Host::default(),
                RunIdentity {
                    boot_id: [1; 16],
                    run_sequence: 1,
                },
                standard_control(),
            )
            .unwrap();
            run_representative(
                &selected,
                &mut second_storage,
                &mut Host::default(),
                RunIdentity {
                    boot_id: [2; 16],
                    run_sequence: 1,
                },
                standard_control(),
            )
            .unwrap();
            serde_json::json!({
                "boot_identity_changed": first_storage.events()[0].run.boot_id != second_storage.events()[0].run.boot_id,
                "plan_identity_changed": first_storage.events()[0].plan != second_storage.events()[0].plan
            })
        }
        "ignored-step-context-error-is-terminal" => ignored_step_error_case(),
        "evidence-reservation-precedes-host-effects" => evidence_reservation_case(),
        other => panic!("unimplemented embedded executor vector `{other}`"),
    }
}

fn maximum_graph_case() -> serde_json::Value {
    let selected = maximum_profile();
    let mut nodes = [NODES[0]; 32];
    for (index, node) in nodes.iter_mut().enumerate() {
        *node = StaticNode {
            semantic_path: MAX_NODE_IDS[index],
            implementation: Id("fixture/rp2040-max-node"),
            input_ports: if (1..=16).contains(&index) { 2 } else { 1 },
            output_ports: if index < 16 { 2 } else { 1 },
            maximum_step_work: 1,
            nesting_depth: MAXIMUM_NESTING,
        };
    }
    let mut cords = [CORDS[0]; 48];
    let mut slot_start = 0_u16;
    for (index, cord) in cords.iter_mut().enumerate() {
        let second_port = index >= 32;
        let local = index % 32;
        let capacity = if second_port { 2 } else { 3 };
        *cord = StaticCord {
            semantic_id: MAX_CORD_IDS[index],
            producer_node: u16::try_from(local).unwrap(),
            producer_port: u8::from(second_port),
            consumer_node: u16::try_from((local + 1) % 32).unwrap(),
            consumer_port: u8::from(second_port),
            slot_start,
            capacity,
            maximum_value_bytes: MAXIMUM_VALUE_BYTES,
        };
        slot_start += capacity;
    }
    let shape = StorageShape::of::<32, 48, 96, 128, 64, 512, 32, 8>();
    let report = validate_static_plan(&plan(&selected, &nodes, &cords), &selected, shape).unwrap();
    serde_json::json!({
        "accepted": true,
        "nodes": report.nodes,
        "cords": report.cords,
        "ports": report.ports,
        "queue_slots": report.queue_slots,
        "within_rp2040_sram": report.static_storage_bytes <= RP2040_SRAM_BYTES
    })
}

struct IgnoredFaultDriver;

impl EmbeddedNode<Host, 16, 1, 1> for IgnoredFaultDriver {
    fn step(&mut self, context: &mut StepContext<'_, Host, 16, 1>) -> EmbeddedStep<1> {
        let _ignored = context.consume(0);
        EmbeddedStep::completed()
    }
}

fn ignored_step_error_case() -> serde_json::Value {
    type FaultStorage = EmbeddedStorage<1, 1, 1, 1, 16, 16, 1, 1>;
    let mut selected = profile();
    selected.maximum_nodes = 1;
    selected.maximum_cords = 1;
    selected.maximum_ports = 1;
    selected.maximum_queue_slots = 1;
    selected.maximum_evidence_records = 16;
    selected.maximum_timers = 1;
    selected.maximum_interests_per_node = 1;
    selected.seal().unwrap();
    let nodes = [StaticNode {
        semantic_path: Id("fixture/ignored-fault"),
        implementation: Id("fixture/rp2040-ignored-fault"),
        input_ports: 0,
        output_ports: 0,
        maximum_step_work: 1,
        nesting_depth: 1,
    }];
    let error = execute_static_plan(
        &plan(&selected, &nodes, &[]),
        &selected,
        &mut FaultStorage::new(),
        &mut [IgnoredFaultDriver],
        &mut Host::default(),
        RunIdentity {
            boot_id: [1; 16],
            run_sequence: 1,
        },
        standard_control(),
    )
    .unwrap_err();
    serde_json::json!({"accepted": false, "code": error.code()})
}

#[derive(Default)]
struct CountingHost {
    calls: u8,
}

impl EmbeddedHostServices<16> for CountingHost {
    fn invoke(&mut self, _binding: u16, _request: EmbeddedValue<16>) -> HostReply<16> {
        self.calls += 1;
        HostReply::Completed(EmbeddedValue::EMPTY)
    }
}

enum ReservationDriver {
    Producer,
    Consumer,
}

impl EmbeddedNode<CountingHost, 16, 2, 1> for ReservationDriver {
    fn step(&mut self, context: &mut StepContext<'_, CountingHost, 16, 2>) -> EmbeddedStep<1> {
        match self {
            Self::Producer => {
                let _ = context.invoke_host(0, EmbeddedValue::EMPTY);
                EmbeddedStep::completed()
            }
            Self::Consumer => EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0))),
        }
    }
}

fn evidence_reservation_case() -> serde_json::Value {
    type ReservationStorage = EmbeddedStorage<2, 1, 2, 1, 16, 6, 1, 1>;
    let mut selected = profile();
    selected.maximum_nodes = 2;
    selected.maximum_cords = 1;
    selected.maximum_ports = 2;
    selected.maximum_queue_slots = 1;
    selected.maximum_evidence_records = 6;
    selected.maximum_timers = 1;
    selected.maximum_interests_per_node = 1;
    selected.seal().unwrap();
    let nodes = [
        StaticNode {
            semantic_path: Id("fixture/reservation-producer"),
            implementation: Id("fixture/rp2040-reservation-producer"),
            input_ports: 0,
            output_ports: 1,
            maximum_step_work: 1,
            nesting_depth: 1,
        },
        StaticNode {
            semantic_path: Id("fixture/reservation-consumer"),
            implementation: Id("fixture/rp2040-reservation-consumer"),
            input_ports: 1,
            output_ports: 0,
            maximum_step_work: 1,
            nesting_depth: 1,
        },
    ];
    let cords = [StaticCord {
        semantic_id: Id("fixture/reservation-cord"),
        producer_node: 0,
        producer_port: 0,
        consumer_node: 1,
        consumer_port: 0,
        slot_start: 0,
        capacity: 1,
        maximum_value_bytes: 1,
    }];
    let mut host = CountingHost::default();
    let error = execute_static_plan(
        &plan(&selected, &nodes, &cords),
        &selected,
        &mut ReservationStorage::new(),
        &mut [ReservationDriver::Producer, ReservationDriver::Consumer],
        &mut host,
        RunIdentity {
            boot_id: [1; 16],
            run_sequence: 1,
        },
        standard_control(),
    )
    .unwrap_err();
    serde_json::json!({
        "accepted": false,
        "code": error.code(),
        "host_calls": host.calls
    })
}

#[derive(Clone, Copy)]
struct TimerDriver {
    deadline: u32,
    waiting: bool,
}

impl EmbeddedNode<Host, 16, 4, 4> for TimerDriver {
    fn step(&mut self, _context: &mut StepContext<'_, Host, 16, 4>) -> EmbeddedStep<4> {
        if self.waiting {
            EmbeddedStep::completed()
        } else {
            self.waiting = true;
            EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Timer(self.deadline)))
        }
    }
}

fn timer_case(deadline: u32) -> serde_json::Value {
    type TimerStorage = EmbeddedStorage<1, 1, 4, 1, 16, 16, 1, 4>;
    let mut selected = profile();
    selected.maximum_nodes = 1;
    selected.maximum_cords = 1;
    selected.maximum_ports = 1;
    selected.maximum_queue_slots = 1;
    selected.maximum_evidence_records = 16;
    selected.maximum_timers = 1;
    selected.seal().unwrap();
    let nodes = [StaticNode {
        semantic_path: Id("fixture/timer"),
        implementation: Id("fixture/rp2040-timer"),
        input_ports: 0,
        output_ports: 0,
        maximum_step_work: 1,
        nesting_depth: 1,
    }];
    let static_plan = plan(&selected, &nodes, &[]);
    let result = execute_static_plan(
        &static_plan,
        &selected,
        &mut TimerStorage::new(),
        &mut [TimerDriver {
            deadline,
            waiting: false,
        }],
        &mut Host::default(),
        RunIdentity {
            boot_id: [1; 16],
            run_sequence: 1,
        },
        RunControl {
            maximum_decisions: 4,
            cancellation_at_decision: None,
            initial_tick: u32::MAX - 1,
        },
    );
    match result {
        Ok(summary) => {
            serde_json::json!({"accepted": true, "status": if summary.status == RunStatus::Succeeded {"succeeded"} else {"other"}})
        }
        Err(error) => serde_json::json!({"accepted": false, "code": error.code()}),
    }
}

fn hil_case(id: &str) -> serde_json::Value {
    let request = HilRequest {
        protocol_version: if id == "foreign-hil-version-rejected" {
            HIL_PROTOCOL_VERSION + 1
        } else {
            HIL_PROTOCOL_VERSION
        },
        nonce: [3; 16],
        expected_plan_hash: PLAN_HASH,
        maximum_decisions: 64,
    };
    let mut encoded = [0; HilRequest::ENCODED_BYTES];
    request.encode(&mut encoded);
    match HilRequest::decode(&encoded) {
        Ok(decoded) => {
            assert_eq!(decoded, request);
            serde_json::json!({"accepted": true})
        }
        Err(error) => serde_json::json!({"accepted": false, "code": error.code()}),
    }
}

fn replacement_case(id: &str) -> serde_json::Value {
    let result = match id {
        "stateful-hot-replacement-rejected" => {
            validate_firmware_replacement(FirmwareReplacementLevel::StatefulHot, 10, 10, 20)
        }
        "quiescent-overlap-must-fit" => {
            validate_firmware_replacement(FirmwareReplacementLevel::Quiescent, 10, 11, 20)
        }
        other => panic!("unimplemented embedded replacement vector `{other}`"),
    };
    match result {
        Ok(()) => serde_json::json!({"accepted": true}),
        Err(error) => serde_json::json!({"accepted": false, "code": error.code()}),
    }
}

#[test]
fn every_embedded_fixture_case_executes() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut executed = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let actual = match case["runner"].as_str().unwrap() {
            "embedded-preflight" => preflight_case(id),
            "embedded-executor" => executor_case(id),
            "embedded-hil-codec" => hil_case(id),
            "embedded-replacement" => replacement_case(id),
            other => panic!("unknown embedded runner `{other}`"),
        };
        assert_eq!(actual, case["expected"], "case `{id}`");
        executed += 1;
    }
    assert_eq!(executed, 28);
}

#[test]
fn wraparound_comparison_has_one_unambiguous_half_range() {
    assert!(deadline_reached(1, 1));
    assert!(deadline_reached(2, 1));
    assert!(!deadline_reached(u32::MAX - 1, 1));
}

#[test]
fn hil_header_and_event_frames_round_trip_exact_attribution() {
    let run = RunIdentity {
        boot_id: [4; 16],
        run_sequence: 9,
    };
    let header = HilRunHeader {
        protocol_version: HIL_PROTOCOL_VERSION,
        nonce: [3; 16],
        plan_hash: PLAN_HASH,
        firmware_identity: SemanticHash::from_bytes([17; 32]),
        capability_report_hash: SemanticHash::from_bytes([18; 32]),
        run,
        status: HilRunStatus::Succeeded,
        decisions: 7,
        evidence_records: 1,
    };
    let mut encoded_header = [0; HilRunHeader::ENCODED_BYTES];
    header.encode(&mut encoded_header);
    assert_eq!(HilRunHeader::decode(&encoded_header).unwrap(), header);

    let frame = HilEventFrame {
        nonce: header.nonce,
        event: EmbeddedEvent {
            plan: PLAN_HASH,
            run,
            sequence: 0,
            tick: u32::MAX,
            subject: EmbeddedSubject::Cord(1),
            kind: EmbeddedEventKind::ValueAccepted,
            value: Some(EmbeddedValue::from_slice(&[1, 2, 3, 4]).unwrap()),
        },
    };
    let mut encoded_frame = [0; HilEventFrame::ENCODED_BYTES];
    frame.encode(&mut encoded_frame).unwrap();
    assert_eq!(HilEventFrame::decode(&encoded_frame).unwrap(), frame);
}

#[test]
fn pico_report_names_exact_build_pools_and_no_unimplemented_zenoh() {
    const REPORTER: PinnedDescriptor<'static> = PinnedDescriptor {
        id: Id("fixture/rp2040-firmware"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([11; 32]),
    };
    const TRUST: PinnedDescriptor<'static> = PinnedDescriptor {
        id: Id("fixture/firmware-build-trust"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([12; 32]),
    };
    const WIFI: PinnedDescriptor<'static> = PinnedDescriptor {
        id: Id("conduit/host.wifi-network"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([13; 32]),
    };
    const POOL: PinnedDescriptor<'static> = PinnedDescriptor {
        id: Id("fixture/rp2040-fixed-pools"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([14; 32]),
    };
    let capabilities = [ReportCapability {
        interface: WIFI,
        mode: Id("ap"),
        subject: Id("cyw43"),
        details: SemanticHash::from_bytes([15; 32]),
        capacity: PlanResourceBudget {
            memory_bytes: 64 * 1024,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 4,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 16 * 1024,
        },
    }];
    let resources = [ReportResource {
        resource: ResourceRef {
            kind: Id("fixture/static-memory-pool"),
            id: Id("sram0"),
        },
        descriptor: POOL,
        capacity: capabilities[0].capacity,
        exclusive: true,
    }];
    let executors = [ExecutorKind::Firmware];
    let targets = [Id("thumbv6m-none-eabi")];
    let abis = [Id("conduit-static-step")];
    let constraints = [profile().identity, SemanticHash::from_bytes([16; 32])];
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("fixture/pico-w-report"),
        host: Id("fixture/pico-w"),
        boot_id: Id("fixture/pico-w-boot"),
        reporter: REPORTER,
        trust: TRUST,
        membership: None,
        time_basis: Id("fixture/boot-ticks"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        available: capabilities[0].capacity,
        capabilities: &capabilities,
        resources: &resources,
        topology: &[],
        execution_placements: &[],
        execution_lanes: &[],
        supported_executors: &executors,
        supported_targets: &targets,
        supported_abis: &abis,
        minimum_plan_version: 0,
        maximum_plan_version: conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
        current_constraints: &constraints,
    };
    let mut scratch = [ZERO; 8];
    report.identity = report.computed_semantic_hash(&mut scratch).unwrap();
    validate_capability_report(
        &report,
        Id("fixture/boot-ticks"),
        12,
        conduit_core::EXECUTION_PLAN_SCHEMA_VERSION,
        &mut scratch,
    )
    .unwrap();
    assert!(
        report
            .capabilities
            .iter()
            .all(|capability| capability.interface.id != Id("conduit/distributed-cord.zenoh"))
    );
}
