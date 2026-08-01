use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    ConfigContract, ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract,
    PlanValidationContext, PortContract, PortFlowConstraints, Presence, ReadyQueueDiscipline,
    SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, SemanticHash, Sensitivity, StopPolicy,
    TemporalContract, TerminalClass, TerminalContract, TypeContractRef, ValueCardinality,
};
use conduit_panel::Node;
use conduit_runtime::{
    AvailabilityState, CompiledInHostService, ExactExecutionReport, ExactRunContext, ExactRunIo,
    ExactRunSessionRegistry, ExactRunState, Handler, HostedServiceStep, HostedServiceStepContext,
    Registry, RunIo, RuntimeError, SchedulerEventKind, SchedulerReservation, Value,
};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const TICKER_SOURCE: &str = "panel 0\n\
node ticker : time/ticker { duration_ticks = 10 time_basis = ref(\"conduit.clock/monotonic-ticks\") maximum_pending = 1 }\n\
node sink : acme/tick-sink\n\
cord ticker.tick -> sink.tick { capacity = 1 max_value_bytes = 32 max_queued_bytes = 32 low_watermark = 0 high_watermark = 1 pressure = block }\n";

const TICK_TEXT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const TICK_SINK_INPUT: PortContract<'static> = PortContract {
    id: Id("tick"),
    direction: Direction::Input,
    value_type: TICK_TEXT,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Committed,
    terminal: TerminalContract::Either,
    sensitivity: Sensitivity::Restricted,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const TICK_SINK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("acme/tick-sink"),
    config: ConfigContract { fields: &[] },
    inputs: &[TICK_SINK_INPUT],
    outputs: &[],
};

static TICK_SINK_COUNT: AtomicUsize = AtomicUsize::new(0);
static TICK_SINK_LAST: AtomicU64 = AtomicU64::new(u64::MAX);

struct TickSink;

impl Handler for TickSink {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [value] = inputs else {
            return Err(RuntimeError::new(
                "CND-RUN-004",
                "tick sink requires one value",
            ));
        };
        if value.value_type != TICK_TEXT {
            return Err(RuntimeError::new(
                "CND-RUN-004",
                "ticker changed its exact value type",
            ));
        }
        let tick = std::str::from_utf8(&value.bytes)
            .ok()
            .and_then(|text| text.strip_suffix('\n'))
            .and_then(|text| text.parse::<u64>().ok())
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "ticker value is not exact text"))?;
        TICK_SINK_LAST.store(tick, Ordering::SeqCst);
        TICK_SINK_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(HostedServiceStep::produced(Vec::new()))
    }
}

fn ticker_registry() -> Registry {
    let mut registry = Registry::hosted_primitives();
    registry.register_contract_only(&TICK_SINK_CONTRACT);
    registry
        .register_compiled_in_host_service(CompiledInHostService {
            contract: &TICK_SINK_CONTRACT,
            implementation_id: "acme/tick-sink-hosted",
            artifact_id: "acme/tick-sink-artifact",
            entrypoint: "tick-sink",
            source_bytes: include_bytes!("standard_time_path.rs"),
            required_authorities: &[],
            factory: || Box::new(TickSink),
            validate_config: |_| Ok(()),
        })
        .unwrap();
    registry
}

fn exact_run(source: &str, run_id: &'static str) -> (Vec<u8>, ExactExecutionReport) {
    let installed = InstalledProfile::observe(source).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let time_node = plan
        .nodes
        .iter()
        .find(|node| node.contract.id.as_str().starts_with("time/"))
        .expect("example has one standard time node");
    let profile = time_node
        .execution_profile
        .expect("time provider has a profile");
    assert_eq!(profile.id.as_str(), "conduit/hosted-time-profile");
    assert_eq!(profile.limits.max_timers, 1);
    assert_eq!(profile.limits.max_retained_values, 1);
    assert_eq!(time_node.allocation.timers, 1);

    let panel = conduit_panel::parse(source).unwrap();
    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let grant_observations = installed.grant_observations(&plan).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let report = resolved
        .run_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 127,
                run_id: Id(run_id),
                grant_observations: &grant_observations,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 512,
                    max_tick: 1024,
                    max_consecutive_yields: 8,
                    max_events: 512,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
        .unwrap();
    assert_eq!(report.terminal, TerminalClass::Succeeded);
    assert!(output.is_empty());
    assert!(error.is_empty());
    (display, report)
}

#[test]
fn all_standard_time_nodes_execute_exactly_with_injected_time() {
    for (source, run_id, expected) in [
        (
            include_str!("../../../examples/time-delay.panel"),
            "run/time/delay",
            "delayed once",
        ),
        (
            include_str!("../../../examples/time-timeout.panel"),
            "run/time/timeout",
            "before timeout",
        ),
        (
            include_str!("../../../examples/time-debounce.panel"),
            "run/time/debounce",
            "settled event",
        ),
        (
            include_str!("../../../examples/time-throttle.panel"),
            "run/time/throttle",
            "admitted request",
        ),
        (
            include_str!("../../../examples/time-compose.panel"),
            "run/time/composition",
            "second",
        ),
    ] {
        let (display, report) = exact_run(source, run_id);
        assert_eq!(display, expected.as_bytes(), "{run_id}");
        assert!(report.scheduler_events.iter().any(|event| event.tick > 0));
        assert_eq!(exact_run(source, run_id), (display, report), "{run_id}");
    }
}

#[test]
fn time_contracts_do_not_claim_an_uninstalled_provider() {
    let contracts = Registry::default();
    let hosted = Registry::hosted_primitives();
    for id in [
        "time/ticker",
        "time/delay",
        "time/timeout",
        "time/debounce",
        "time/throttle",
    ] {
        assert_eq!(
            contracts.node_availability(id).state,
            AvailabilityState::ContractOnly
        );
        assert_eq!(
            hosted.node_availability(id).state,
            AvailabilityState::ProviderAvailable
        );
    }
}

#[test]
fn ticker_emits_again_after_each_exact_clock_wake_without_new_epoch() {
    TICK_SINK_COUNT.store(0, Ordering::SeqCst);
    TICK_SINK_LAST.store(u64::MAX, Ordering::SeqCst);
    let registry = ticker_registry();
    let installed = InstalledProfile::observe_registry(TICKER_SOURCE, &registry).unwrap();
    let document = compile_source(TICKER_SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let ticker = plan
        .nodes
        .iter()
        .find(|node| node.contract.id.as_str() == "time/ticker")
        .expect("ticker is present in the exact plan");
    assert_eq!(
        ticker.execution_profile.unwrap().id.as_str(),
        "conduit/hosted-time-profile"
    );
    assert_eq!(ticker.allocation.timers, 1);
    let ticker_contract = conduit_std::standard_node_contract("time/ticker").unwrap();
    assert_eq!(ticker_contract.outputs[0].values.as_str(), "zero-or-more");
    assert_eq!(ticker_contract.outputs[0].terminal.as_str(), "open-ended");

    let panel = conduit_panel::parse(TICKER_SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let grants = installed.grant_observations(&plan).unwrap();
    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let mut session = resolved
        .start_exact_session(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 128,
                run_id: Id("run/time/ticker"),
                grant_observations: &grants,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 64,
                    max_tick: 64,
                    max_consecutive_yields: 8,
                    max_events: 64,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();

    for _ in 0..8 {
        if session.state() != ExactRunState::Active {
            break;
        }
        session.pump(1, &[]).unwrap();
    }
    assert_eq!(session.state(), ExactRunState::Waiting);
    assert_eq!(session.next_timer_deadline(), Some(12));
    assert_eq!(TICK_SINK_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(TICK_SINK_LAST.load(Ordering::SeqCst), 0);
    let identity = session.identity().clone();
    let first_tick_events = session
        .scheduler_events()
        .filter(|event| matches!(event.kind, SchedulerEventKind::NodeOutcome { .. }))
        .count();

    session.advance_to(12, &[]).unwrap();
    for _ in 0..8 {
        if session.state() != ExactRunState::Active {
            break;
        }
        session.pump(1, &[]).unwrap();
    }
    assert_eq!(session.state(), ExactRunState::Waiting);
    assert_eq!(session.next_timer_deadline(), Some(23));
    assert_eq!(session.identity(), &identity);
    assert_eq!(TICK_SINK_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(TICK_SINK_LAST.load(Ordering::SeqCst), 1);
    assert!(
        session
            .scheduler_events()
            .filter(|event| matches!(event.kind, SchedulerEventKind::NodeOutcome { .. }))
            .count()
            > first_tick_events,
        "the exact ticker provider must take another bounded production step"
    );

    session.cancel(StopPolicy::Abort).unwrap();
    assert_eq!(
        session.state(),
        ExactRunState::Terminal(TerminalClass::Cancelled)
    );
    session.finalize().unwrap();
    assert_eq!(sessions.active_sessions(), 0);
}

#[test]
fn stale_missing_unbounded_and_silently_lossy_time_profiles_fail_resolution() {
    let delay = include_str!("../../../examples/time-delay.panel");
    for (source, code) in [
        (
            delay.replace("    clock = ref(\"conduit.clock/monotonic-ticks\")\n", ""),
            "CND-TIM-010",
        ),
        (
            delay.replace(
                "6b9c687226d4a1965e780b63b4bdc0922de2a686c3c1365f4f68f7219f30cc48",
                "7b9c687226d4a1965e780b63b4bdc0922de2a686c3c1365f4f68f7219f30cc48",
            ),
            "CND-TIM-011",
        ),
        (
            delay.replace("duration_ticks = 3", "duration_ticks = 1000001"),
            "CND-TIM-012",
        ),
        (
            include_str!("../../../examples/time-debounce.panel")
                .replace("loss = \"coalesce\"", "loss = \"implicit\""),
            "CND-TIM-012",
        ),
        (
            include_str!("../../../examples/time-throttle.panel")
                .replace("overflow = \"block\"", "overflow = \"coalesce\""),
            "CND-TIM-012",
        ),
    ] {
        let error = InstalledProfile::observe(&source)
            .err()
            .expect("time profile must fail closed");
        assert_eq!(error.code, code, "{}", error.message);
    }
    let error = match InstalledProfile::observe_registry(
        &TICKER_SOURCE.replace("duration_ticks = 10", "duration_ticks = 0"),
        &ticker_registry(),
    ) {
        Ok(_) => panic!("zero ticker interval must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.code, "CND-TIM-012", "{}", error.message);
}

#[test]
fn pending_terminal_policies_and_trailing_coalescing_are_executable() {
    let delay_drop = include_str!("../../../examples/time-delay.panel")
        .replace("terminal = \"drain\"", "terminal = \"drop\"");
    assert!(exact_run(&delay_drop, "run/time/delay-drop").0.is_empty());

    let debounce_drop = include_str!("../../../examples/time-debounce.panel")
        .replace("terminal = \"flush\"", "terminal = \"drop\"");
    assert!(
        exact_run(&debounce_drop, "run/time/debounce-drop")
            .0
            .is_empty()
    );

    let throttle_trailing = include_str!("../../../examples/time-throttle.panel")
        .replace("mode = \"leading\"", "mode = \"trailing\"")
        .replace("overflow = \"block\"", "overflow = \"coalesce\"");
    assert_eq!(
        exact_run(&throttle_trailing, "run/time/throttle-coalesce").0,
        b"admitted request"
    );
    let throttle_drop = throttle_trailing.replace("terminal = \"flush\"", "terminal = \"drop\"");
    assert!(
        exact_run(&throttle_drop, "run/time/throttle-drop")
            .0
            .is_empty()
    );
}
