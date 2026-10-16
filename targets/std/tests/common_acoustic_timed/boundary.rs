//! Fixture boundary around the authored parent Plan and production Gear registry.
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
    KernelCompositeHost, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;
pub fn prepare(
    plan: Plan,
    front: &CheckedFront,
    registry: &KernelOperationRegistry,
) -> KernelCompositeHost {
    let fragment = &plan.fragments[0];
    let bind = |port: &PortDescriptor| {
        let selected = fragment
            .fore_ports
            .iter()
            .find(|selected| {
                selected.front_port_id == port.port_id && selected.direction == port.direction
            })
            .unwrap();
        KernelCompositeFrontBinding {
            external_port: port.clone(),
            internal_child: fragment.host_id.clone(),
            internal_placement_id: selected.placement_id.clone(),
            internal_port_id: selected.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        }
    };
    // This wrapper is a named fixture boundary, not a new DSP Kind or Back.
    // The inner plan selects only the production expression and pair factories.
    let offer = BackOfferBuilder::new(
        Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "fixture/acoustic-timed-parent".into(),
            kind_contract_revision: "fixture/acoustic-timed-parent@1".into(),
            inputs: front.inputs().to_vec(),
            outputs: front.outputs().to_vec(),
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(
                front.value_contracts().to_vec(),
            )],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 32768,
            },
        },
        Back {
            capability_id: "fixture/acoustic-timed-parent@1".into(),
            execution_profile_id: "fixture/acoustic-timed-parent@1".into(),
            implementation_id: "fixture/acoustic-timed-parent@1".into(),
            artifact_id: "fixture/acoustic-timed-parent@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/acoustic-timed-parent@1".into(),
        boundary: KernelCompositeBoundary {
            input_fronts: front.inputs().iter().map(bind).collect(),
            output_fronts: front.outputs().iter().map(bind).collect(),
        },
        external_capability: offer,
        internal_plan: plan,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };

    KernelCompositeHost::prepare_with_sign_storage(
        definition,
        registry,
        conduit_composite::KernelCompositeSignStorage {
            additional_local_items: 256,
            additional_remote_items: 128,
        },
    )
    .unwrap()
}
