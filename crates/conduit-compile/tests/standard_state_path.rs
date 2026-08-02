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
    for node in plan
        .nodes
        .iter()
        .filter(|node| node.contract.id.as_str().starts_with("state/"))
    {
        let profile = node
            .execution_profile
            .expect("state provider has a profile");
        assert_eq!(profile.id.as_str(), "conduit/hosted-state-profile");
        assert_eq!(
            profile.limits.max_retained_values,
            conduit_std::STATE_MAX_ENTRIES as u16
        );
        assert_eq!(
            profile.limits.max_retained_bytes,
            conduit_std::STATE_MAX_VALUE_BYTES
        );
        assert_eq!(node.allocation.timers, 0);
    }

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
                plan_epoch: 128,
                run_id: Id(run_id),
                grant_observations: &grant_observations,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 1024,
                    max_tick: 1024,
                    max_consecutive_yields: 8,
                    max_events: 1024,
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

fn cancel_exact(source: &str, run_id: &'static str) -> ExactExecutionReport {
    let installed = InstalledProfile::observe(source).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
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
        .cancel_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 128,
                run_id: Id(run_id),
                grant_observations: &grant_observations,
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 256,
                    max_tick: 512,
                    max_consecutive_yields: 8,
                    max_events: 256,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            conduit_core::StopPolicy::Abort,
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
        .unwrap();
    assert!(output.is_empty());
    assert!(error.is_empty());
    assert!(display.is_empty());
    report
}

#[test]
fn cell_deduplicate_and_cache_execute_exactly_and_repeat() {
    for (source, run_id, expected) in [
        (
            include_str!("../../../examples/state-cell.panel"),
            "run/state/cell",
            "initial,first,second",
        ),
        (
            include_str!("../../../examples/state-deduplicate.panel"),
            "run/state/deduplicate",
            "one,two",
        ),
        (
            include_str!("../../../examples/state-cache.panel"),
            "run/state/cache",
            "stored,alpha,invalidated,miss",
        ),
        (
            include_str!("../../../examples/state-compose.panel"),
            "run/state/composition",
            "stored,alpha",
        ),
    ] {
        let (display, report) = exact_run(source, run_id);
        assert_eq!(display, expected.as_bytes(), "{run_id}");
        assert_eq!(exact_run(source, run_id), (display, report), "{run_id}");
    }
}

#[test]
fn fifo_eviction_is_visible_for_deduplicate_and_cache() {
    let deduplicate = include_str!("../../../examples/state-deduplicate.panel")
        .replace("one\\none\\ntwo\\none\\n", "one\\ntwo\\nthree\\none\\n");
    assert_eq!(
        exact_run(&deduplicate, "run/state/deduplicate-fifo").0,
        b"one,two,three,one"
    );

    let cache = include_str!("../../../examples/state-cache.panel")
        .replace(
            "put:a=alpha\\nget:a\\ninvalidate:a\\nget:a\\n",
            "put:a=alpha\\nput:b=beta\\nput:c=gamma\\nget:a\\nget:b\\nget:c\\n",
        )
        .replace("maximum_entries = 4", "maximum_entries = 2");
    assert_eq!(
        exact_run(&cache, "run/state/cache-fifo").0,
        b"stored,stored,stored,miss,beta,gamma"
    );
}

#[test]
fn stale_unbounded_persistent_and_unknown_state_profiles_fail_closed() {
    for (source, code) in [
        (
            include_str!("../../../examples/state-cell.panel").replace(
                "69d3f4d8d53741fd075be4e6755af9be19db951a5065b89be5a2e87c7755565e",
                "79d3f4d8d53741fd075be4e6755af9be19db951a5065b89be5a2e87c7755565e",
            ),
            "CND-STA-011",
        ),
        (
            include_str!("../../../examples/state-deduplicate.panel")
                .replace("maximum_entries = 2", "maximum_entries = 17"),
            "CND-STA-012",
        ),
        (
            include_str!("../../../examples/state-cache.panel")
                .replace("checkpoint = \"unsupported\"", "checkpoint = \"automatic\""),
            "CND-STA-012",
        ),
        (
            include_str!("../../../examples/state-cache.panel")
                .replace("ttl = \"none\"", "ttl = \"ambient\""),
            "CND-STA-012",
        ),
    ] {
        let error = InstalledProfile::observe(&source)
            .err()
            .expect("state profile must fail closed");
        assert_eq!(error.code, code, "{}", error.message);
    }
}

#[test]
fn state_contracts_do_not_claim_an_uninstalled_provider() {
    let contracts = Registry::default();
    let hosted = Registry::hosted_primitives();
    for id in ["state/cell", "state/deduplicate", "state/cache"] {
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
fn cell_get_reset_and_abort_cancellation_are_explicit() {
    let cell = include_str!("../../../examples/state-cell.panel")
        .replace("maximum_items = 4", "maximum_items = 8")
        .replace(
            "join:",
            "command_source: std/literal { value = \"get\\nreset\\n\" }\n\
             commands: std/text/lines { maximum_line_bytes = 64 maximum_retained_prefix_bytes = 64 }\n\
             join:",
        )
        .replace(
            "cell.current >",
            "command_source.value > commands.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }\n\
             commands.line > cell.command { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }\n\
             cell.current >",
        );
    assert_eq!(
        exact_run(&cell, "run/state/cell-get-reset").0,
        b"initial,initial,initial,first,second"
    );

    for (source, run_id) in [
        (
            include_str!("../../../examples/state-cell.panel"),
            "run/state/cell-cancel",
        ),
        (
            include_str!("../../../examples/state-deduplicate.panel"),
            "run/state/deduplicate-cancel",
        ),
        (
            include_str!("../../../examples/state-cache.panel"),
            "run/state/cache-cancel",
        ),
    ] {
        assert_eq!(
            cancel_exact(source, run_id).terminal,
            TerminalClass::Cancelled
        );
    }
}
