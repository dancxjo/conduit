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
            terminal_transductions: [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
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
    assert!(module.contains(&generated.render_terminal_transductions()));
}

#[test]
fn terminal_renderer_preserves_plural_enum_payloads_without_core_types() {
    use conduit_kernel::scheduler::*;
    let mut output = String::new();
    let laws = [
        AssignedTerminalTransduction {
            input: conduit_kernel::PortId(0),
            output: conduit_kernel::PortId(1),
            normal_close: AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(
                AssignedFiniteTerminalEmission {
                    maximum_items: 2,
                    maximum_bytes: 91,
                },
            ),
            abnormal: AssignedAbnormalTransduction::DomainSpecific { law: [7; 32] },
            cancellation: AssignedCancellationTransduction::Request {
                input: conduit_kernel::PortId(2),
                disposition_kind: [9; 32],
            },
        },
        AssignedTerminalTransduction {
            input: conduit_kernel::PortId(1),
            output: conduit_kernel::PortId(1),
            normal_close: AssignedNormalCloseTransduction::Consume,
            abnormal: AssignedAbnormalTransduction::Recover,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        },
    ];
    // Reuse the renderer's expression helper directly, rather than inventing a
    // second serialization for the bounded laws.
    for law in laws {
        output.push_str(&super::render::terminal_expression(Some(law)));
    }
    assert!(output.contains("AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(conduit_kernel::scheduler::AssignedFiniteTerminalEmission"));
    assert!(output.contains("maximum_items: 2, maximum_bytes: 91"));
    assert!(output
        .contains("AssignedCancellationTransduction::Request { input: conduit_kernel::PortId(2)"));
    assert!(output.contains("AssignedAbnormalTransduction::Recover"));
    assert!(!output.contains("conduit_core"));
}
