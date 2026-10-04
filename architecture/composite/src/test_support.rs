//! Shared sealed single-child geometry for execution contract fixtures.
use crate::*;
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;

#[path = "../tests/support/allocation.rs"]
pub(crate) mod allocation;
pub(crate) mod common {
    use alloc::vec::Vec;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/tests/common/sealed_state.rs"
    ));
}

pub(crate) fn single_child(
    mut fragment: PlanFragment,
    maximum_bytes: u32,
) -> KernelCompositeDefinition {
    fragment.states.clear();
    fragment.expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let placement = &mut fragment.placements[0];
    placement.limits.max_queue_bytes = maximum_bytes;
    let front = |port: &PortDescriptor| KernelCompositeFrontBinding {
        external_port: port.clone(),
        internal_child: fragment.host_id.clone(),
        internal_placement_id: placement.placement_id.clone(),
        internal_port_id: port.port_id.clone(),
        terminal: CompositeFrontTerminal::Independent,
    };
    let input = placement.inputs[0].clone();
    let output = placement.outputs[0].clone();
    let boundary = KernelCompositeBoundary {
        input_fronts: vec![front(&input)],
        output_fronts: vec![front(&output)],
    };
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/execution".into(),
        external_capability: capability_offer_from_parts! {
            semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
            capability_id: "fixture/execution".into(), kind_id: kind_id("fixture/execution"),
            kind_contract_revision: "fixture/execution@1".into(),
            implementation: ImplementationOffer {
                execution_profile_id: "fixture/execution@1".into(),
                implementation_id: "fixture/execution@1".into(), artifact_id: "fixture/execution@1".into(),
            },
            inputs: vec![input], outputs: vec![output], host_calls: vec![],
            resource_requirements: vec![], authority_requirements: vec![],
            limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: maximum_bytes },
        },
        internal_plan: common::seal(fragment),
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    assert!(verify_plan(&definition.internal_plan));
    definition
}
