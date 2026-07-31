use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    TerminalClass,
};
use conduit_runtime::{
    AvailabilityState, ExactExecutionReport, ExactRunContext, Registry, RunIo, RuntimeError,
    SchedulerReservation,
};

fn run_result(
    source: &str,
    run_id: &'static str,
) -> Result<(Vec<u8>, ExactExecutionReport), RuntimeError> {
    let installed = InstalledProfile::observe(source)?;
    let document = compile_source(source, &installed.input)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    let arena = Bump::new();
    let plan = document
        .as_plan(&arena)
        .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
    for node in plan
        .nodes
        .iter()
        .filter(|node| node.contract.id.as_str().starts_with("supervision/"))
    {
        let profile = node
            .execution_profile
            .expect("supervision provider has a profile");
        assert_eq!(profile.id.as_str(), "conduit/hosted-supervision-profile");
        assert_eq!(profile.limits.max_timers, 2);
        assert_eq!(profile.limits.max_retained_values, 4);
        assert_eq!(node.allocation.timers, 2);
        assert_eq!(node.allocation.checkpoints, 0);
    }

    let panel = conduit_panel::parse(source).unwrap();
    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan)?;
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let report = resolved.run_exact_report(
        &plan,
        &bindings,
        ExactRunContext {
            semantic_source_hash: plan.source_semantic_hash,
            plan_epoch: 129,
            run_id: Id(run_id),
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
    )?;
    assert!(output.is_empty());
    assert!(error.is_empty());
    Ok((display, report))
}

fn exact_run(source: &str, run_id: &'static str) -> (Vec<u8>, ExactExecutionReport) {
    let result = run_result(source, run_id).unwrap();
    assert_eq!(result.1.terminal, TerminalClass::Succeeded);
    result
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
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    resolved
        .cancel_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 129,
                run_id: Id(run_id),
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
        .unwrap()
}

#[test]
fn retry_breaker_and_composition_execute_exactly_and_repeat() {
    for (source, run_id, expected) in [
        (
            include_str!("../../../examples/supervision-retry.panel"),
            "run/supervision/retry",
            "job,job",
        ),
        (
            include_str!("../../../examples/supervision-circuit-breaker.panel"),
            "run/supervision/circuit-breaker",
            "first,probe",
        ),
        (
            include_str!("../../../examples/supervision-compose.panel"),
            "run/supervision/composition",
            "job,job",
        ),
    ] {
        let result = exact_run(source, run_id);
        assert_eq!(result.0, expected.as_bytes(), "{run_id}");
        assert_eq!(exact_run(source, run_id), result, "{run_id}");
    }
}

#[test]
fn retry_permissions_exhaustion_entropy_and_descriptors_fail_closed() {
    let retry = include_str!("../../../examples/supervision-retry.panel");
    for (source, code) in [
        (
            retry.replace(
                "idempotency = \"idempotent\"",
                "idempotency = \"forbidden\"",
            ),
            "CND-SVP-006",
        ),
        (
            retry.replace(
                "eligible-failure\\nsuccess\\n",
                "committed-failure\\nsuccess\\n",
            ),
            "CND-SVP-007",
        ),
        (
            retry.replace(
                "eligible-failure\\nsuccess\\n",
                "eligible-failure\\neligible-failure\\neligible-failure\\n",
            ),
            "CND-SVP-005",
        ),
        (
            retry
                .replace("jitter = \"none\"", "jitter = \"injected\"")
                .replace("jitter_ticks = 0", "jitter_ticks = 1"),
            "CND-SVP-008",
        ),
    ] {
        assert_eq!(
            run_result(&source, "run/supervision/failure")
                .unwrap_err()
                .code,
            code
        );
    }

    for (source, code) in [
        (
            retry.replace(
                "354ceb6902073d56e0cd8ea1ee1aa11372fcecab51db988677c9c5b569d141c6",
                "454ceb6902073d56e0cd8ea1ee1aa11372fcecab51db988677c9c5b569d141c6",
            ),
            "CND-SVP-011",
        ),
        (
            retry.replace("maximum_attempts = 3", "maximum_attempts = 9"),
            "CND-SVP-012",
        ),
        (
            retry.replace("checkpoint = \"unsupported\"", "checkpoint = \"automatic\""),
            "CND-SVP-012",
        ),
    ] {
        assert_eq!(
            InstalledProfile::observe(&source)
                .err()
                .expect("invalid supervision profile must fail")
                .code,
            code
        );
    }
}

#[test]
fn supervision_contracts_are_honest_and_backoff_is_one_policy() {
    let contracts = Registry::default();
    let hosted = Registry::hosted_primitives();
    for id in ["supervision/retry", "supervision/circuit-breaker"] {
        assert_eq!(
            contracts.node_availability(id).state,
            AvailabilityState::ContractOnly
        );
        assert_eq!(
            hosted.node_availability(id).state,
            AvailabilityState::ProviderAvailable
        );
    }
    assert_eq!(
        contracts.node_availability("supervision/backoff").state,
        AvailabilityState::Unsupported
    );
    let retry = conduit_std::standard_node_contract("supervision/retry").unwrap();
    assert!(
        retry
            .config
            .fields
            .iter()
            .any(|field| field.key.as_str() == "backoff")
    );
    assert!(conduit_std::standard_node_contract("supervision/backoff").is_none());
}

#[test]
fn retry_and_breaker_abort_cancellation_are_bounded() {
    for (source, run_id) in [
        (
            include_str!("../../../examples/supervision-retry.panel"),
            "run/supervision/retry-cancel",
        ),
        (
            include_str!("../../../examples/supervision-circuit-breaker.panel"),
            "run/supervision/breaker-cancel",
        ),
    ] {
        assert_eq!(
            cancel_exact(source, run_id).terminal,
            TerminalClass::Cancelled
        );
    }
}
