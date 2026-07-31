use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    TerminalClass,
};
use conduit_runtime::{
    AvailabilityState, ExactExecutionReport, ExactRunContext, Registry, RunIo, SchedulerReservation,
};

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
