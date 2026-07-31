use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    TerminalClass,
};
use conduit_runtime::{AvailabilityState, ExactRunContext, Registry, RunIo, SchedulerReservation};

fn exact_report(
    source: &str,
    run_id: &'static str,
) -> (Vec<u8>, Vec<u8>, conduit_runtime::ExactExecutionReport) {
    let installed = InstalledProfile::observe(source).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();

    for node in plan.nodes.iter().filter(|node| {
        matches!(
            node.contract.id.as_str(),
            "conduit.std/tee"
                | "conduit.std/merge"
                | "conduit.std/zip"
                | "conduit.std/gate"
                | "conduit.std/select"
        )
    }) {
        let profile = node.execution_profile.expect("flow provider has a profile");
        assert_eq!(
            profile.id.as_str(),
            "conduit/hosted-structural-flow-profile"
        );
        assert_eq!(profile.limits.max_input_leases, 2);
        assert_eq!(profile.limits.max_output_reservations, 2);
        assert_eq!(profile.limits.max_retained_values, 2);
        assert_eq!(profile.limits.max_retained_bytes, 2048);
        assert!(profile.limits.implementation_memory_bytes >= 8192);
    }
    assert!(plan.budget.evidence_bytes >= 256 * 1024);

    let registry = Registry::hosted_primitives();
    let panel = conduit_panel::parse(source).unwrap();
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
                plan_epoch: 124,
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
                display: &mut Vec::new(),
            },
        )
        .unwrap_or_else(|error| panic!("{run_id}: {error}"));
    assert_eq!(report.terminal, TerminalClass::Succeeded);
    assert!(!report.evidence.is_empty());
    assert!(report.evidence_bytes <= plan.budget.evidence_bytes);
    (output, error, report)
}

fn exact_run(source: &str, run_id: &'static str) -> (Vec<u8>, Vec<u8>) {
    let (output, error, _) = exact_report(source, run_id);
    (output, error)
}

fn cancel_exact(source: &str, run_id: &'static str) -> conduit_runtime::ExactExecutionReport {
    let installed = InstalledProfile::observe(source).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let registry = Registry::hosted_primitives();
    let panel = conduit_panel::parse(source).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let report = resolved
        .cancel_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 124,
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
                display: &mut Vec::new(),
            },
        )
        .unwrap();
    assert!(output.is_empty(), "{run_id}");
    assert!(error.is_empty(), "{run_id}");
    report
}

#[test]
fn all_five_standard_flow_nodes_use_exact_plans_and_the_production_executor() {
    for (source, run_id, expected) in [
        (
            include_str!("../../../examples/flow-tee.panel"),
            "run/conduit.std/tee",
            (
                b"one value, two coupled branches".as_slice(),
                b"one value, two coupled branches".as_slice(),
            ),
        ),
        (
            include_str!("../../../examples/flow-merge.panel"),
            "run/conduit.std/merge",
            (b"firstsecond".as_slice(), b"".as_slice()),
        ),
        (
            include_str!("../../../examples/flow-zip.panel"),
            "run/conduit.std/zip",
            (b"left".as_slice(), b"right".as_slice()),
        ),
        (
            include_str!("../../../examples/flow-gate.panel"),
            "run/conduit.std/gate",
            (b"admitted".as_slice(), b"".as_slice()),
        ),
        (
            include_str!("../../../examples/flow-select.panel"),
            "run/conduit.std/select",
            (b"selected".as_slice(), b"".as_slice()),
        ),
    ] {
        let actual = exact_run(source, run_id);
        assert_eq!(actual.0, expected.0, "{run_id}");
        assert_eq!(actual.1, expected.1, "{run_id}");
    }
}

#[test]
fn unsupported_flow_profiles_fail_during_resolution() {
    for (kind, config) in [
        ("conduit.std/tee", r#"mode = "hidden-copy""#),
        ("conduit.std/merge", r#"ordering = "wake-order""#),
        ("conduit.std/zip", r#"unpaired = "unbounded-retain""#),
        ("conduit.std/gate", r#"retained = "unbounded""#),
        ("conduit.std/select", r#"inactive = "drop""#),
    ] {
        let source = format!("panel 0\nnode invalid : {kind} {{ {config} }}\n");
        let error = Registry::hosted_primitives()
            .resolve(&conduit_panel::parse(&source).unwrap())
            .unwrap_err();
        assert_eq!(error.code, "CND-IMP-001", "{kind}: {error}");
    }

    let contract_only = Registry::default();
    let installed = Registry::hosted_primitives();
    for id in [
        "conduit.std/tee",
        "conduit.std/merge",
        "conduit.std/zip",
        "conduit.std/gate",
        "conduit.std/select",
    ] {
        assert_eq!(
            contract_only.node_availability(id).state,
            AvailabilityState::ContractOnly
        );
        assert_eq!(
            installed.node_availability(id).state,
            AvailabilityState::ProviderAvailable
        );
    }
}

#[test]
fn checked_standalone_and_composition_panels_execute_exactly() {
    let cases = [
        (
            include_str!("../../../examples/flow-tee.panel"),
            "run/flow/example-tee",
            b"one value, two coupled branches".as_slice(),
            b"one value, two coupled branches".as_slice(),
        ),
        (
            include_str!("../../../examples/flow-merge.panel"),
            "run/flow/example-merge",
            b"firstsecond".as_slice(),
            b"".as_slice(),
        ),
        (
            include_str!("../../../examples/flow-zip.panel"),
            "run/flow/example-zip",
            b"left".as_slice(),
            b"right".as_slice(),
        ),
        (
            include_str!("../../../examples/flow-gate.panel"),
            "run/flow/example-gate",
            b"admitted".as_slice(),
            b"".as_slice(),
        ),
        (
            include_str!("../../../examples/flow-select.panel"),
            "run/flow/example-select",
            b"selected".as_slice(),
            b"".as_slice(),
        ),
        (
            include_str!("../../../examples/flow-compose.panel"),
            "run/flow/example-compose",
            b"right".as_slice(),
            b"right".as_slice(),
        ),
    ];

    for (source, run_id, expected_output, expected_error) in cases {
        let (output, error) = exact_run(source, run_id);
        assert_eq!(output, expected_output, "{run_id}");
        assert_eq!(error, expected_error, "{run_id}");
    }
}

#[test]
fn every_standard_flow_node_cancels_before_work_with_bounded_evidence() {
    for (source, run_id) in [
        (
            include_str!("../../../examples/flow-tee.panel"),
            "run/conduit.std/tee/cancel",
        ),
        (
            include_str!("../../../examples/flow-merge.panel"),
            "run/conduit.std/merge/cancel",
        ),
        (
            include_str!("../../../examples/flow-zip.panel"),
            "run/conduit.std/zip/cancel",
        ),
        (
            include_str!("../../../examples/flow-gate.panel"),
            "run/conduit.std/gate/cancel",
        ),
        (
            include_str!("../../../examples/flow-select.panel"),
            "run/conduit.std/select/cancel",
        ),
    ] {
        let report = cancel_exact(source, run_id);
        assert_eq!(report.terminal, TerminalClass::Cancelled, "{run_id}");
        assert!(!report.evidence.is_empty(), "{run_id}");
    }
}

#[test]
fn merge_round_robin_survives_capacity_one_pressure_and_repeats_deterministically() {
    let source = r#"
panel 0
node left_chunks : std/literal { value = "a1\na2\n" }
node right_chunks : std/literal { value = "b1\nb2\n" }
node left_lines : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node right_lines : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node merged : conduit.std/merge { ordering = "round-robin" }
node joined : std/text/join { separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }
node encoded : text/encode-utf8
node sink : io/stdout
cord left_chunks.value -> left_lines.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord right_chunks.value -> right_lines.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord left_lines.line -> merged.left { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord right_lines.line -> merged.right { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord merged.value -> joined.item { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord joined.text -> encoded.text { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
cord encoded.bytes -> sink.bytes { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
"#;
    let first = exact_report(source, "run/conduit.std/merge/pressure");
    let second = exact_report(source, "run/conduit.std/merge/pressure");
    assert_eq!((&first.0, &first.1), (&b"a1b1a2b2".to_vec(), &Vec::new()));
    assert_eq!((&second.0, &second.1), (&first.0, &first.1));
    assert_eq!(second.2.allocation, first.2.allocation);
    assert_eq!(second.2.high_water, first.2.high_water);
    assert_eq!(second.2.scheduler_events, first.2.scheduler_events);
    assert_eq!(second.2.evidence, first.2.evidence);
}

#[test]
fn tee_modes_preserve_all_values_across_uneven_capacity_one_branches() {
    for mode in ["coupled", "isolated"] {
        let source = format!(
            r#"
panel 0
node chunks : std/literal {{ value = "a\nb\nc\n" }}
node lines : std/text/lines {{ maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }}
node split : conduit.std/tee {{ mode = "{mode}" }}
node left_joined : std/text/join {{ separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }}
node right_joined : std/text/join {{ separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }}
node left_encoded : text/encode-utf8
node right_encoded : text/encode-utf8
node left_sink : io/stdout
node right_sink : io/stderr
cord chunks.value -> lines.text {{ capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }}
cord lines.line -> split.value {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord split.left -> left_joined.item {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord split.right -> right_joined.item {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord left_joined.text -> left_encoded.text {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_joined.text -> right_encoded.text {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord left_encoded.bytes -> left_sink.bytes {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_encoded.bytes -> right_sink.bytes {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
"#
        );
        assert_eq!(
            exact_run(
                &source,
                if mode == "coupled" {
                    "run/conduit.std/tee/coupled-pressure"
                } else {
                    "run/conduit.std/tee/isolated-pressure"
                }
            ),
            (b"abc".to_vec(), b"abc".to_vec()),
            "{mode}"
        );
    }
}

#[test]
fn gate_and_select_apply_control_before_data_without_hidden_loss() {
    let gate = r#"
panel 0
node data_chunks : std/literal { value = "d1\nd2\n" }
node command_chunks : std/literal { value = "open\nclosed\nopen\n" }
node data : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node commands : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node gated : conduit.std/gate { initial = "closed" retained = "block" }
node joined : std/text/join { separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }
node encoded : text/encode-utf8
node sink : io/stdout
cord data_chunks.value -> data.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord command_chunks.value -> commands.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord data.line -> gated.candidate { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord commands.line -> gated.permit { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord gated.admitted -> joined.item { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord joined.text -> encoded.text { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
cord encoded.bytes -> sink.bytes { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
"#;
    assert_eq!(
        exact_run(gate, "run/conduit.std/gate/toggle-race"),
        (b"d1d2".to_vec(), Vec::new())
    );

    let select = r#"
panel 0
node left_chunks : std/literal { value = "l1\nl2\n" }
node right_chunks : std/literal { value = "r1\n" }
node command_chunks : std/literal { value = "right\nleft\n" }
node left : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node right : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node commands : std/text/lines { maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }
node selected : conduit.std/select { initial = "left" inactive = "block" }
node joined : std/text/join { separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }
node encoded : text/encode-utf8
node sink : io/stdout
cord left_chunks.value -> left.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord right_chunks.value -> right.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord command_chunks.value -> commands.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }
cord left.line -> selected.left { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord right.line -> selected.right { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord commands.line -> selected.selector { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord selected.selected -> joined.item { capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }
cord joined.text -> encoded.text { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
cord encoded.bytes -> sink.bytes { capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }
"#;
    assert_eq!(
        exact_run(select, "run/conduit.std/select/toggle-race"),
        (b"r1l1l2".to_vec(), Vec::new())
    );
}

#[test]
fn zip_has_explicit_uneven_terminal_policies() {
    let source = |policy: &str| {
        format!(
            r#"
panel 0
node left_chunks : std/literal {{ value = "l1\nl2\nl3\n" }}
node right_chunks : std/literal {{ value = "r1\nr2\n" }}
node left_lines : std/text/lines {{ maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }}
node right_lines : std/text/lines {{ maximum_line_bytes = 16 maximum_retained_prefix_bytes = 16 }}
node paired : conduit.std/zip {{ unpaired = "{policy}" }}
node left_joined : std/text/join {{ separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }}
node right_joined : std/text/join {{ separator = "" maximum_items = 8 maximum_item_bytes = 16 maximum_separator_bytes = 1 maximum_output_bytes = 128 }}
node left_encoded : text/encode-utf8
node right_encoded : text/encode-utf8
node left_sink : io/stdout
node right_sink : io/stderr
cord left_chunks.value -> left_lines.text {{ capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_chunks.value -> right_lines.text {{ capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }}
cord left_lines.line -> paired.left {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_lines.line -> paired.right {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord paired.left -> left_joined.item {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord paired.right -> right_joined.item {{ capacity = 1 max_value_bytes = 16 max_queued_bytes = 16 low_watermark = 0 high_watermark = 1 pressure = block }}
cord left_joined.text -> left_encoded.text {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_joined.text -> right_encoded.text {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord left_encoded.bytes -> left_sink.bytes {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
cord right_encoded.bytes -> right_sink.bytes {{ capacity = 1 max_value_bytes = 128 max_queued_bytes = 128 low_watermark = 0 high_watermark = 1 pressure = block }}
"#
        )
    };
    assert_eq!(
        exact_run(&source("drop"), "run/conduit.std/zip/unpaired-drop"),
        (b"l1l2".to_vec(), b"r1r2".to_vec())
    );

    let failing = source("fail");
    let installed = InstalledProfile::observe(&failing).unwrap();
    let document = compile_source(&failing, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let registry = Registry::hosted_primitives();
    let panel = conduit_panel::parse(&failing).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let failure = resolved
        .run_exact_report(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 124,
                run_id: Id("run/conduit.std/zip/unpaired-fail"),
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
                display: &mut Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(failure.code, "conduit.std/zip-unpaired");
}
