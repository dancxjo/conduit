use conduit_compile::{InstalledHostObservationInput, InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION,
    SchedulerDecisionReason, SchedulerPolicy, StopPolicy, TerminalClass,
};
use conduit_runtime::{
    ExactRunContext, ExactRunIo, ExactRunSessionRegistry, ExactRunState, Registry, RunIo,
    SchedulerEventKind, SchedulerReservation, SchedulerSubject,
};
use std::process::Command;

const SOURCE: &str = r#"panel 0
first: std/literal { value = "first" }
second: std/literal { value = "second" }
third: std/literal { value = "third" }
first_display: display/text
second_display: display/text
third_display: display/text
effect_text: std/literal { value = "effect" }
effect_bytes: std/data/encode-utf8 { codec = ref("conduit.codec/utf-8") codec_schema_version = 0 codec_hash = bytes("f219297cb276bc91eccddb346a8b21e7edd4414b8844014108513747ae11bf53") maximum_input_bytes = 64 maximum_output_bytes = 64 }
effect_sink: io/stdout
first.value > first_display.text
second.value > second_display.text
third.value > third_display.text
effect_text.value > effect_bytes.text
effect_bytes.bytes > effect_sink.bytes
"#;

fn context<'a>(
    plan: &'a conduit_core::ExecutionPlan<'a>,
    grants: &'a [conduit_runtime::ExactGrantObservation],
    run_id: &'a str,
) -> ExactRunContext<'a> {
    ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: 1,
        run_id: Id(run_id),
        grant_observations: grants,
        validation: PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 64,
            max_tick: 128,
            max_consecutive_yields: 8,
            max_events: 128,
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    }
}

#[test]
fn canonical_runs_use_three_hosted_lanes_and_match_the_serial_oracle() {
    let conformance = include_str!("../../../conformance/c4/portable-execution.json");
    for id in [
        "production-arranged-three-lane-overlap",
        "production-persistent-session-hosted-lanes",
        "production-hosted-lanes-match-serial-oracle",
        "production-hosted-lanes-fill-proposal-capacity",
        "production-hosted-lane-loss-fails-session",
        "production-hosted-cancellation-fences-provider",
        "production-cross-lane-wake-reaches-serialized-sinks",
        "canonical-conduct-projects-hosted-batch",
    ] {
        assert!(conformance.contains(&format!("\"id\":\"{id}\"")));
    }
    let registry = Registry::hosted_primitives();
    let mut host = InstalledHostObservationInput::conduct_host();
    host.execution_lanes.truncate(3);
    let mut installed =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[]).unwrap();
    installed.input.execution_arrangement.maximum_proposal_bytes = 17;
    installed.input.seal().unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let arrangement = document.execution_arrangement().unwrap();
    let panel = conduit_panel::parse(SOURCE).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let grants = installed.grant_observations(&plan).unwrap();

    let mut arranged_input = &b""[..];
    let mut arranged_output = Vec::new();
    let mut arranged_error = Vec::new();
    let mut arranged_display = Vec::new();
    let arranged = resolved
        .run_exact_report_arranged(
            &plan,
            &arrangement,
            &bindings,
            context(&plan, &grants, "fixture/arranged"),
            &mut RunIo {
                input: &mut arranged_input,
                output: &mut arranged_output,
                error: &mut arranged_error,
                display: &mut arranged_display,
            },
        )
        .unwrap();
    let lane_batch = arranged.hosted_lane_batch.as_ref().unwrap();
    assert_eq!(lane_batch.committed_tickets, [1, 2, 3]);
    assert_eq!(lane_batch.proposal_slots_used, 3);
    assert_eq!(lane_batch.proposal_bytes_used, 17);
    assert_eq!(lane_batch.proposal_bytes_capacity, 17);
    assert_eq!(lane_batch.commit_domain, arrangement.commit_domains[0].id);
    assert_eq!(lane_batch.physical_completion_order.len(), 3);
    let release = lane_batch.physical_completion_order[0].release_sequence;
    assert!(
        lane_batch
            .physical_completion_order
            .iter()
            .all(|observation| {
                observation.entered_sequence < release
                    && release < observation.finished_sequence
                    && observation.release_sequence == release
                    && !observation.faulted
            })
    );

    let mut serial_input = &b""[..];
    let mut serial_output = Vec::new();
    let mut serial_error = Vec::new();
    let mut serial_display = Vec::new();
    let serial = resolved
        .run_exact_report(
            &plan,
            &bindings,
            context(&plan, &grants, "fixture/serial"),
            &mut RunIo {
                input: &mut serial_input,
                output: &mut serial_output,
                error: &mut serial_error,
                display: &mut serial_display,
            },
        )
        .unwrap();
    assert_eq!(arranged.summary, serial.summary);
    assert_eq!(arranged.terminal, serial.terminal);
    assert_eq!(arranged.scheduler_events, serial.scheduler_events);
    assert_eq!(arranged_output, serial_output);
    assert_eq!(arranged_output, b"effect");
    assert_eq!(arranged_display, serial_display);
    assert_eq!(arranged_display, b"firstsecondthird");
    assert!(serial.hosted_lane_batch.is_none());
    let cross_lane_targets = arrangement
        .boundaries
        .iter()
        .filter_map(|boundary| {
            let from = arrangement
                .regions
                .iter()
                .find(|region| region.id == boundary.from_region)?;
            let to = arrangement
                .regions
                .iter()
                .find(|region| region.id == boundary.to_region)?;
            (from.lane != to.lane).then_some(boundary)?;
            let cord = plan
                .cords
                .iter()
                .find(|cord| cord.id.as_str() == boundary.cord)?;
            plan.nodes
                .iter()
                .position(|node| node.instance == cord.to.node)
                .and_then(|index| u16::try_from(index).ok())
        })
        .collect::<Vec<_>>();
    assert!(!cross_lane_targets.is_empty());
    assert!(arranged.scheduler_events.iter().any(|event| {
        matches!(event.subject, SchedulerSubject::Node(node) if cross_lane_targets.contains(&node))
            && event.kind
                == SchedulerEventKind::NodeWoken {
                    reason: SchedulerDecisionReason::InputReady,
                }
    }));

    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let mut persistent = resolved
        .start_exact_session_arranged(
            &plan,
            &arrangement,
            &bindings,
            context(&plan, &grants, "fixture/persistent"),
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();
    while !matches!(persistent.state(), ExactRunState::Terminal(_)) {
        persistent.pump(16, &[]).unwrap();
    }
    assert_eq!(
        persistent.state(),
        ExactRunState::Terminal(TerminalClass::Succeeded)
    );
    assert_eq!(
        persistent.hosted_lane_batch().unwrap().committed_tickets,
        [1, 2, 3]
    );
    persistent.with_io(|io| assert_eq!(io.display(), b"firstsecondthird"));
    persistent.with_io(|io| assert_eq!(io.output(), b"effect"));
    persistent.finalize().unwrap();

    let mut cancelled = resolved
        .start_exact_session_arranged(
            &plan,
            &arrangement,
            &bindings,
            context(&plan, &grants, "fixture/cancelled"),
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();
    cancelled.cancel(StopPolicy::Abort).unwrap();
    while !matches!(cancelled.state(), ExactRunState::Terminal(_)) {
        cancelled.pump(16, &[]).unwrap();
    }
    assert_eq!(
        cancelled.state(),
        ExactRunState::Terminal(TerminalClass::Cancelled)
    );
    assert!(cancelled.hosted_lane_batch().is_none());
    cancelled.with_io(|io| assert!(io.display().is_empty()));
    cancelled.finalize().unwrap();

    let mut lost = resolved
        .start_exact_session_arranged(
            &plan,
            &arrangement,
            &bindings,
            context(&plan, &grants, "fixture/lane-loss"),
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();
    lost.observe_hosted_lane_loss(1).unwrap();
    let failure = lost.pump(16, &[]).unwrap_err();
    assert_eq!(failure.code, "CND-LAN-005");
    assert_eq!(lost.state(), ExactRunState::Terminal(TerminalClass::Failed));
    assert!(lost.hosted_lane_batch().is_none());
    lost.finalize().unwrap();
}

#[test]
fn canonical_conduct_ndjson_projects_the_production_hosted_batch() {
    let path = std::env::temp_dir().join(format!(
        "conduct-production-hosted-lanes-{}.panel",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .args(["--run", "--format=ndjson"])
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let batch = records
        .iter()
        .find(|record| record["record"] == "hosted_lane_batch")
        .map(|record| &record["hosted_lane_batch"])
        .unwrap();
    let observations = batch["physical_completion_order"].as_array().unwrap();
    assert!(observations.len() >= 3);
    assert_eq!(batch["active_lanes"].as_array().map(Vec::len), Some(3));
    assert!(observations.iter().all(|observation| {
        observation["entered_sequence"].as_u64().unwrap()
            < observation["release_sequence"].as_u64().unwrap()
            && observation["release_sequence"].as_u64().unwrap()
                < observation["finished_sequence"].as_u64().unwrap()
    }));
    assert!(
        batch["committed_tickets"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .all(|(index, ticket)| ticket.as_u64() == Some(u64::try_from(index).unwrap() + 1))
    );
}
