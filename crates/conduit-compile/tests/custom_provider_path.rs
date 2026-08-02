use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    ConfigContract, Id, NodeContract, PlanValidationContext, ReadyQueueDiscipline,
    SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, StopPolicy, TerminalClass,
};
use conduit_panel::Node;
use conduit_runtime::{
    CompiledInHostService, ExactHostedRunSession, ExactRunContext, ExactRunIo,
    ExactRunSessionRegistry, ExactRunState, Handler, HandlerFactory, HostedServiceCleanup,
    HostedServiceInterest, HostedServiceStep, HostedServiceStepContext, Registry, RunIo,
    RuntimeError, SchedulerReservation, Value,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const CUSTOM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("acme/weather/probe"),
    config: ConfigContract { fields: &[] },
    inputs: &[],
    outputs: &[],
};

const CLEANUP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("acme/weather/cleanup"),
    config: ConfigContract { fields: &[] },
    inputs: &[],
    outputs: &[],
};

struct CustomWeatherProvider;

impl Handler for CustomWeatherProvider {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        assert!(inputs.is_empty());
        Ok(Vec::new())
    }
}

struct LongLivedWeatherProvider {
    wakes_seen: u8,
}

static LIVE_PROVIDER_PREPARES: AtomicUsize = AtomicUsize::new(0);
static LIVE_PROVIDER_STARTS: AtomicUsize = AtomicUsize::new(0);
static LIVE_PROVIDER_CLEANUPS: AtomicUsize = AtomicUsize::new(0);

impl Handler for LongLivedWeatherProvider {
    fn prepare(
        &mut self,
        _node: &Node,
        binding: conduit_runtime::ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        assert_eq!(binding.instance, "root/weather");
        LIVE_PROVIDER_PREPARES.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn start(&mut self, _node: &Node) -> Result<(), RuntimeError> {
        LIVE_PROVIDER_STARTS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        assert!(inputs.is_empty());
        if self.wakes_seen == 2 {
            return Ok(HostedServiceStep::completed(Vec::new()));
        }
        self.wakes_seen += 1;
        Ok(HostedServiceStep::waiting(
            HostedServiceInterest::HostOperation {
                subject: Id("acme.weather/refresh"),
            },
        ))
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        _context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        LIVE_PROVIDER_CLEANUPS.fetch_add(1, Ordering::SeqCst);
        Ok(HostedServiceCleanup::Complete)
    }
}

struct DeferredCleanupProvider {
    cleanup_polled: bool,
}

impl Handler for DeferredCleanupProvider {
    fn step(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        Ok(HostedServiceStep::waiting(
            HostedServiceInterest::HostOperation {
                subject: Id("acme.weather/work"),
            },
        ))
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        _context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        if self.cleanup_polled {
            Ok(HostedServiceCleanup::Complete)
        } else {
            self.cleanup_polled = true;
            Ok(HostedServiceCleanup::waiting(
                HostedServiceInterest::HostOperation {
                    subject: Id("acme.weather/cleanup"),
                },
            ))
        }
    }
}

struct StuckCleanupProvider;

impl Handler for StuckCleanupProvider {
    fn step(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        Ok(HostedServiceStep::waiting(
            HostedServiceInterest::HostOperation {
                subject: Id("acme.weather/work"),
            },
        ))
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        _context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        Ok(HostedServiceCleanup::waiting(
            HostedServiceInterest::HostOperation {
                subject: Id("acme.weather/cleanup"),
            },
        ))
    }
}

fn start_cleanup_session(
    factory: HandlerFactory,
    run_id: &'static str,
) -> (ExactHostedRunSession, ExactRunSessionRegistry, u64) {
    const SOURCE: &str = "panel 0\nweather: acme/weather/cleanup\n";
    let mut registry = Registry::hosted_primitives();
    registry.register_contract_only(&CLEANUP_CONTRACT);
    registry
        .register_compiled_in_host_service(CompiledInHostService {
            contract: &CLEANUP_CONTRACT,
            implementation_id: "acme/implementation/weather-cleanup",
            artifact_id: "acme/artifact/weather-cleanup",
            entrypoint: "weather-cleanup",
            source_bytes: include_bytes!("custom_provider_path.rs"),
            required_authorities: &[],
            factory,
            validate_config: |_| Ok(()),
        })
        .unwrap();
    let installed = InstalledProfile::observe_registry(SOURCE, &registry).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let cancellation_ticks = plan.nodes[0]
        .execution_profile
        .expect("hosted cleanup provider has an execution profile")
        .limits
        .cancellation_ticks;
    let panel = conduit_panel::parse(SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let mut session = resolved
        .start_exact_session(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 154,
                run_id: Id(run_id),
                grant_observations: &[],
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 32,
                    max_tick: cancellation_ticks.checked_add(8).unwrap(),
                    max_consecutive_yields: 4,
                    max_events: 32,
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
    while session.state() == ExactRunState::Active {
        session.pump(1, &[]).unwrap();
    }
    assert_eq!(session.state(), ExactRunState::Waiting);
    (session, sessions, cancellation_ticks)
}

#[test]
fn custom_namespaced_node_survives_source_plan_binding_execution_and_evidence() {
    const SOURCE: &str = "panel 0\nweather: acme/weather/probe\n";
    let mut registry = Registry::hosted_primitives();
    registry.register_contract_only(&CUSTOM_CONTRACT);
    registry
        .register_compiled_in_host_service(CompiledInHostService {
            contract: &CUSTOM_CONTRACT,
            implementation_id: "acme/implementation/weather-native-v1",
            artifact_id: "acme/artifact/weather-native-v1",
            entrypoint: "weather-probe",
            source_bytes: include_bytes!("custom_provider_path.rs"),
            required_authorities: &[],
            factory: || Box::new(CustomWeatherProvider),
            validate_config: |_| Ok(()),
        })
        .unwrap();

    let parsed = conduit_panel::parse(SOURCE).unwrap();
    registry.resolve(&parsed).unwrap();
    let installed = InstalledProfile::observe_registry(SOURCE, &registry).unwrap();
    let mut missing_descriptor = installed.input.clone();
    missing_descriptor
        .catalog
        .nodes
        .retain(|node| node.id != CUSTOM_CONTRACT.id.as_str());
    assert_eq!(missing_descriptor.seal().unwrap_err().code(), "CND-CMP-002");
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    assert_eq!(plan.nodes.len(), 1);
    assert_eq!(plan.nodes[0].contract.id, CUSTOM_CONTRACT.id);
    assert_eq!(
        plan.nodes[0].implementation.id,
        Id("acme/implementation/weather-native-v1")
    );

    let panel = conduit_panel::parse(SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let report = resolved
        .run_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 152,
                run_id: Id("run/acme-weather/1"),
                grant_observations: &[],
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 16,
                    max_tick: 32,
                    max_consecutive_yields: 4,
                    max_events: 16,
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
                display: &mut Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(report.summary.nodes_completed, 1);
    assert_eq!(report.terminal, conduit_core::TerminalClass::Succeeded);
    let node_evidence = report
        .evidence
        .iter()
        .find(|record| record.node_id.is_some())
        .expect("exact node evidence is retained");
    assert_eq!(
        node_evidence.semantic_contract_id.as_deref(),
        Some(CUSTOM_CONTRACT.id.as_str())
    );
    let contract_hash = plan.nodes[0].contract.semantic_hash.to_string();
    assert_eq!(
        node_evidence.semantic_contract_descriptor_hash.as_deref(),
        Some(contract_hash.as_str())
    );
    assert!(!report.evidence.is_empty());
    assert!(output.is_empty());
    assert!(error.is_empty());
}

#[test]
fn custom_hosted_provider_waits_across_multiple_exact_host_wakes() {
    const SOURCE: &str = "panel 0\nweather: acme/weather/probe\n";
    LIVE_PROVIDER_PREPARES.store(0, Ordering::SeqCst);
    LIVE_PROVIDER_STARTS.store(0, Ordering::SeqCst);
    LIVE_PROVIDER_CLEANUPS.store(0, Ordering::SeqCst);
    let mut registry = Registry::hosted_primitives();
    registry.register_contract_only(&CUSTOM_CONTRACT);
    registry
        .register_compiled_in_host_service(CompiledInHostService {
            contract: &CUSTOM_CONTRACT,
            implementation_id: "acme/implementation/weather-live-native",
            artifact_id: "acme/artifact/weather-live-native",
            entrypoint: "weather-live-probe",
            source_bytes: include_bytes!("custom_provider_path.rs"),
            required_authorities: &[],
            factory: || Box::new(LongLivedWeatherProvider { wakes_seen: 0 }),
            validate_config: |_| Ok(()),
        })
        .unwrap();

    let installed = InstalledProfile::observe_registry(SOURCE, &registry).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let panel = conduit_panel::parse(SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let mut session = resolved
        .start_exact_session(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 153,
                run_id: Id("run/acme-weather/live"),
                grant_observations: &[],
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 16,
                    max_tick: 32,
                    max_consecutive_yields: 4,
                    max_events: 16,
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

    while session.state() == ExactRunState::Active {
        session.pump(1, &[]).unwrap();
    }
    assert_eq!(session.state(), ExactRunState::Waiting);
    let identity = session.identity().clone();
    assert_eq!(
        session
            .notify_host_operation(Id("acme.weather/wrong-subject"), &[])
            .unwrap()
            .state,
        ExactRunState::Waiting
    );
    for _ in 0..2 {
        assert_eq!(
            session
                .notify_host_operation(Id("acme.weather/refresh"), &[])
                .unwrap()
                .state,
            ExactRunState::Active
        );
        while session.state() == ExactRunState::Active {
            session.pump(1, &[]).unwrap();
        }
    }
    assert_eq!(
        session.state(),
        ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
    );
    assert_eq!(session.identity(), &identity);
    session.finalize().unwrap();
    assert_eq!(sessions.active_sessions(), 0);
    assert_eq!(LIVE_PROVIDER_PREPARES.load(Ordering::SeqCst), 1);
    assert_eq!(LIVE_PROVIDER_STARTS.load(Ordering::SeqCst), 1);
    assert_eq!(LIVE_PROVIDER_CLEANUPS.load(Ordering::SeqCst), 1);
}

#[test]
fn abort_waits_for_one_exact_cleanup_wake_before_terminal_cancellation() {
    let (mut session, sessions, _) = start_cleanup_session(
        || {
            Box::new(DeferredCleanupProvider {
                cleanup_polled: false,
            })
        },
        "run/acme-weather/deferred-cleanup",
    );
    let identity = session.identity().clone();
    assert_eq!(
        session.cancel(StopPolicy::Abort).unwrap().state,
        ExactRunState::Aborting
    );
    assert_eq!(session.pump(1, &[]).unwrap().state, ExactRunState::Aborting);
    assert_eq!(
        session
            .notify_host_operation(Id("acme.weather/cleanup"), &[])
            .unwrap()
            .state,
        ExactRunState::Aborting
    );
    assert_eq!(
        session.pump(1, &[]).unwrap().state,
        ExactRunState::Terminal(TerminalClass::Cancelled)
    );
    assert_eq!(session.identity(), &identity);
    assert!(session.exact_evidence().iter().any(|record| {
        record.event_kind == "terminal" && record.terminal_cause == Some("cancelled")
    }));
    session.finalize().unwrap();
    assert_eq!(sessions.active_sessions(), 0);
}

#[test]
fn hosted_cleanup_timeout_fails_the_same_exact_epoch() {
    let (mut session, sessions, cancellation_ticks) = start_cleanup_session(
        || Box::new(StuckCleanupProvider),
        "run/acme-weather/cleanup-timeout",
    );
    let identity = session.identity().clone();
    let cancelled = session.cancel(StopPolicy::Abort).unwrap();
    assert_eq!(cancelled.state, ExactRunState::Aborting);
    assert_eq!(session.pump(1, &[]).unwrap().state, ExactRunState::Aborting);
    let expired_tick = cancelled
        .tick
        .checked_add(cancellation_ticks)
        .and_then(|tick| tick.checked_add(1))
        .unwrap();
    let error = session.advance_to(expired_tick, &[]).unwrap_err();
    assert_eq!(error.code, "CND-RUN-013");
    assert_eq!(
        session.state(),
        ExactRunState::Terminal(TerminalClass::Failed)
    );
    assert_eq!(session.identity(), &identity);
    session.finalize().unwrap();
    assert_eq!(sessions.active_sessions(), 0);
}
