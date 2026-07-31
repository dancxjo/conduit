use bumpalo::Bump;
use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::{
    ConfigContract, Id, NodeContract, PlanValidationContext, ReadyQueueDiscipline,
    SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
};
use conduit_panel::Node;
use conduit_runtime::{
    CompiledInHostService, ExactRunContext, Handler, Registry, RunIo, RuntimeError,
    SchedulerReservation, Value,
};

const CUSTOM_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("acme/weather/probe"),
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

#[test]
fn custom_namespaced_node_survives_source_plan_binding_execution_and_evidence() {
    const SOURCE: &str = "panel 3\nnode weather : acme/weather/probe\n";
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
    assert_eq!(missing_descriptor.seal().unwrap_err().code(), "CND-CMP-004");
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
    assert!(!report.evidence.is_empty());
    assert!(output.is_empty());
    assert!(error.is_empty());
}
