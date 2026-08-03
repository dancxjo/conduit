use conduit_compile::{InstalledHostObservationInput, InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    TerminalClass,
};
use conduit_runtime::{
    ExactRunContext, ExactRunIo, ExactRunSessionRegistry, ExactRunState, Registry, RunIo,
    SchedulerReservation,
};

const SOURCE: &str = r#"panel 0
first: std/literal { value = "first" }
second: std/literal { value = "second" }
third: std/literal { value = "third" }
first_display: display/text
second_display: display/text
third_display: display/text
first.value > first_display.text
second.value > second_display.text
third.value > third_display.text
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
    ] {
        assert!(conformance.contains(&format!("\"id\":\"{id}\"")));
    }
    let registry = Registry::hosted_primitives();
    let mut host = InstalledHostObservationInput::conduct_host();
    host.execution_lanes.truncate(3);
    let mut installed =
        InstalledProfile::observe_registry_on_host(SOURCE, &registry, &host, &[]).unwrap();
    installed.input.execution_arrangement.maximum_proposal_bytes = 16;
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
    assert_eq!(lane_batch.proposal_bytes_used, 16);
    assert_eq!(lane_batch.proposal_bytes_capacity, 16);
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
    assert_eq!(arranged_display, serial_display);
    assert_eq!(arranged_display, b"firstsecondthird");
    assert!(serial.hosted_lane_batch.is_none());

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
    persistent.finalize().unwrap();
}
