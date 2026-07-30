use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
};
use conduit_runtime::{AvailabilityState, ExactRunContext, Registry, RunIo, SchedulerReservation};

const SOURCE: &str = include_str!("../../../examples/text-lines-join.panel");

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
