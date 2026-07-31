use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    StepOutcomeKind,
};
use conduit_runtime::{
    AvailabilityState, ExactRunContext, Registry, RunIo, SchedulerEventKind, SchedulerReservation,
    SchedulerSubject,
};

const SOURCE: &str = include_str!("../../../examples/text-lines-join.panel");

fn maximum_branch(name: &str, value: &str, separator: &str) -> String {
    format!(
        r#"
node chunks_{name} : std/literal {{
    value = "{value}"
}}
node lines_{name} : std/text/lines {{
    maximum_line_bytes = 1024
    maximum_retained_prefix_bytes = 1024
}}
node joined_{name} : std/text/join {{
    separator = "{separator}"
    maximum_items = 8
    maximum_item_bytes = 1024
    maximum_separator_bytes = 64
    maximum_output_bytes = 4096
}}

cord chunks_{name}.value -> lines_{name}.text {{
    capacity = 1
    max_value_bytes = 4096
    max_queued_bytes = 4096
    low_watermark = 0
    high_watermark = 1
    pressure = block
}}
cord lines_{name}.line -> joined_{name}.item {{
    capacity = 8
    max_value_bytes = 1024
    max_queued_bytes = 8192
    low_watermark = 1
    high_watermark = 8
    pressure = block
}}
"#
    )
}

const MAXIMUM_OUTPUT: &str = r#"
node encoded : std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 4096 maximum_output_bytes = 4096 }
node output : io/stdout
cord joined_a.text -> encoded.text {
    capacity = 1
    max_value_bytes = 4096
    max_queued_bytes = 4096
    low_watermark = 0
    high_watermark = 1
    pressure = block
}
cord encoded.bytes -> output.bytes {
    capacity = 1
    max_value_bytes = 4096
    max_queued_bytes = 4096
    low_watermark = 0
    high_watermark = 1
    pressure = block
}
"#;

#[test]
fn text_lines_and_join_survive_exact_plan_and_production_execution() {
    let installed = InstalledProfile::observe(SOURCE).unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    for id in ["std/text/lines", "std/text/join"] {
        let node = plan
            .nodes
            .iter()
            .find(|node| node.contract.id.as_str() == id)
            .unwrap();
        let profile = node.execution_profile.unwrap();
        assert!(profile.limits.max_step_work > 0);
        assert!(profile.limits.implementation_memory_bytes > 0);
        assert_eq!(
            profile.limits.max_retained_values,
            conduit_std::JOIN_MAX_ITEMS as u16
        );
        assert_eq!(
            profile.limits.max_retained_bytes,
            (conduit_std::JOIN_MAX_ITEMS * conduit_std::JOIN_MAX_ITEM_BYTES) as u64
        );
        assert_eq!(
            profile.limits.max_output_bytes,
            conduit_std::JOIN_MAX_OUTPUT_BYTES as u64
        );
    }
    assert!(plan.budget.evidence_bytes >= 256 * 1024);
    assert_eq!(plan.cords[1].flow.capacity.items(), 8);
    assert_eq!(plan.cords[1].queue_memory_bytes, 8192);

    let panel = conduit_panel::parse(SOURCE).unwrap();
    let registry = Registry::hosted_primitives();
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
                plan_epoch: 126,
                run_id: Id("run/text-lines-join/1"),
                grant_observations: &[],
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
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(report.terminal, conduit_core::TerminalClass::Succeeded);
    let expected_items = ["alpha", "beta", "", "gamma"];
    let mut reference = [0; conduit_std::JOIN_MAX_OUTPUT_BYTES];
    let reference_length =
        conduit_std::join_text_into(&expected_items, " | ", &mut reference).unwrap();
    assert_eq!(output, reference[..reference_length]);
    assert!(error.is_empty());
    assert!(!report.evidence.is_empty());

    let mut cancelled_input = &b""[..];
    let mut cancelled_output = Vec::new();
    let mut cancelled_error = Vec::new();
    let cancelled = resolved
        .cancel_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                run_id: Id("run/text-lines-join/cancel"),
                ..ExactRunContext {
                    semantic_source_hash: plan.source_semantic_hash,
                    plan_epoch: 126,
                    run_id: Id("run/text-lines-join/cancel-base"),
                    grant_observations: &[],
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
                }
            },
            conduit_core::StopPolicy::Abort,
            &mut RunIo {
                input: &mut cancelled_input,
                output: &mut cancelled_output,
                error: &mut cancelled_error,
                display: &mut Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(cancelled.terminal, conduit_core::TerminalClass::Cancelled);
    assert!(cancelled_output.is_empty());

    let contract_only = Registry::default();
    for id in ["std/text/lines", "std/text/join"] {
        assert_eq!(
            contract_only.node_availability(id).state,
            AvailabilityState::ContractOnly
        );
        assert_eq!(
            registry.node_availability(id).state,
            AvailabilityState::ProviderAvailable
        );
    }
}

#[test]
fn maximum_join_requeues_fairly_with_lines_under_tiny_steps() {
    let mut items = vec!["x".repeat(127); conduit_std::JOIN_MAX_ITEMS];
    *items.last_mut().unwrap() = "x".repeat(128);
    let value = items.join("\\n");
    assert_eq!(value.replace("\\n", "\n").len(), 1024);
    let separator = "|";
    let source = format!(
        "panel 0\n{}\n{MAXIMUM_OUTPUT}",
        maximum_branch("a", &value, separator)
    );
    let mut installed = InstalledProfile::observe(&source).unwrap();
    installed.input.plan_budget.memory_bytes = 16 * 1024 * 1024;
    installed.input.plan_budget.evidence_bytes = 4 * 1024 * 1024;
    installed.input.seal().unwrap();
    let document = compile_source(&source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();

    for id in ["std/text/lines", "std/text/join"] {
        let profile = plan
            .nodes
            .iter()
            .find(|node| node.contract.id.as_str() == id)
            .unwrap()
            .execution_profile
            .unwrap();
        assert_eq!(profile.limits.max_step_work, 8);
    }

    let panel = conduit_panel::parse(&source).unwrap();
    let registry = Registry::hosted_primitives();
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
                plan_epoch: 189,
                run_id: Id("run/text-lines-join/maximum"),
                grant_observations: &[],
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 8_192,
                    max_tick: 16_384,
                    max_consecutive_yields: 8,
                    max_events: 20_000,
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

    assert_eq!(report.terminal, conduit_core::TerminalClass::Succeeded);
    assert_eq!(output.len(), 1017 + (conduit_std::JOIN_MAX_ITEMS - 1));
    assert!(error.is_empty());

    let node = |instance: &str| {
        plan.nodes
            .iter()
            .position(|node| node.instance.as_str().ends_with(instance))
            .unwrap() as u16
    };
    let progress_positions = |target| {
        report
            .scheduler_events
            .iter()
            .enumerate()
            .filter_map(|(position, event)| {
                (event.subject == SchedulerSubject::Node(target)
                    && matches!(
                        event.kind,
                        SchedulerEventKind::NodeOutcome {
                            outcome: StepOutcomeKind::Progress
                        }
                    ))
                .then_some(position)
            })
            .collect::<Vec<_>>()
    };
    let lines_progress = progress_positions(node("lines_a"));
    let join_progress = progress_positions(node("joined_a"));
    assert!(
        lines_progress.len() > 100,
        "maximum line did not incrementally requeue"
    );
    assert!(
        join_progress.len() > 100,
        "maximum join did not incrementally requeue"
    );
    assert!(
        join_progress[0] < *lines_progress.last().unwrap(),
        "join was not scheduled while lines remained ready"
    );
    assert!(
        lines_progress.windows(2).any(|window| join_progress
            .iter()
            .any(|position| window[0] < *position && *position < window[1])),
        "round-robin scheduling never ran join between incremental lines steps"
    );
}

#[test]
fn maximum_line_scanning_and_copying_span_bounded_steps() {
    let value = "x".repeat(conduit_std::LINES_MAX_LINE_BYTES);
    let source = format!(
        "panel 0\n{}\n{MAXIMUM_OUTPUT}",
        maximum_branch("a", &value, "|")
    );
    let mut installed = InstalledProfile::observe(&source).unwrap();
    installed.input.plan_budget.memory_bytes = 8 * 1024 * 1024;
    installed.input.plan_budget.evidence_bytes = 2 * 1024 * 1024;
    installed.input.seal().unwrap();
    let document = compile_source(&source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let lines_index = plan
        .nodes
        .iter()
        .position(|node| node.instance.as_str().ends_with("lines_a"))
        .unwrap() as u16;
    let profile = plan.nodes[lines_index as usize].execution_profile.unwrap();
    assert_eq!(profile.limits.max_step_work, 8);

    let panel = conduit_panel::parse(&source).unwrap();
    let registry = Registry::hosted_primitives();
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
                plan_epoch: 189,
                run_id: Id("run/text-lines/maximum"),
                grant_observations: &[],
                validation: PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 4_096,
                    max_tick: 8_192,
                    max_consecutive_yields: 8,
                    max_events: 10_000,
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

    assert_eq!(report.terminal, conduit_core::TerminalClass::Succeeded);
    assert_eq!(output.len(), conduit_std::LINES_MAX_LINE_BYTES);
    assert!(error.is_empty());
    let progress_steps = report
        .scheduler_events
        .iter()
        .filter(|event| {
            event.subject == SchedulerSubject::Node(lines_index)
                && matches!(
                    event.kind,
                    SchedulerEventKind::NodeOutcome {
                        outcome: StepOutcomeKind::Progress
                    }
                )
        })
        .count();
    assert!(
        progress_steps > 2 * conduit_std::LINES_MAX_LINE_BYTES / 8,
        "maximum line was not scanned and copied across bounded scheduler steps"
    );
}
