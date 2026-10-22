//! Fixed Rust table rendering remains independent of native admission metadata.
use crate::{
    GeneratedConfigurationEntry, GeneratedConfigurationValue, GeneratedEmbeddedPlan, GeneratedPort,
    GeneratedStaticNode, GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
};
use conduit_core::{CancellationPolicy, TerminalPolicy};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

#[test]
fn renderer_emits_fixed_current_kernel_tables() {
    let generated = GeneratedEmbeddedPlan {
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        plan_id: "plan-1".to_owned(),
        fragment_id: "fragment-1".to_owned(),
        host_id: "host-1".to_owned(),
        boot_id: "boot-1".to_owned(),
        offer_generation: 7,
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        nodes: vec![GeneratedStaticNode {
            node: 0,
            placement_id: "pulse".to_owned(),
            kind_id: "flow/pulse".to_owned(),
            implementation_id: "signal/pulse".to_owned(),
            artifact_id: "artifact/pulse".to_owned(),
            input_cords: [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
            maximum_step_fuel: 2,
        }],
        input_ports: Vec::new(),
        output_ports: vec![GeneratedPort {
            node: 0,
            port: 0,
            port_id: "out".to_owned(),
            value_kind: "value/signal".to_owned(),
        }],
        configuration: vec![GeneratedConfigurationEntry {
            node: 0,
            key: "count".to_owned(),
            value: GeneratedConfigurationValue::U64(16),
        }],
        cords: Vec::new(),
        remote_endpoints: Vec::new(),
        routes: Vec::new(),
        route_targets: Vec::new(),
        host_calls: Vec::new(),
        native_bindings: Vec::new(),
        resources: Vec::new(),
        resource_cords: Vec::new(),
        signs: Vec::new(),
        startup_dependencies: Vec::new(),
        startup_order: vec![0],
        expected_terminals: Vec::new(),
        cord_value_slots: 0,
        cord_value_bytes: 0,
        sign_items: 1,
        sign_bytes: 16,
    };

    let module = generated.render_rust_module();
    assert!(module.contains("pub const PLAN_ID: &str = \"plan-1\";"));
    assert!(module.contains("conduit_kernel::scheduler::NodeSpec<16>"));
    assert!(module.contains("conduit_core::ConfigurationValue::U64(16)"));
    assert!(module.contains("pub const GENERATED_STARTUP_ORDER"));
    assert!(module.contains("CancelAllAndRejectLateCompletion"));
    assert!(!module.contains("ExecutionPlan"));
}
