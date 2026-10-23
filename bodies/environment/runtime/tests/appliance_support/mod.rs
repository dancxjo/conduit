use conduit_core::*;
use conduit_environment_runtime::construction::ApplianceSource;
use conduit_pulse_capture::{installation::ReadyPulseBase, PulseReadContract};

pub fn advertisement(
    _source: &ApplianceSource,
    software: &mut conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
    ready: &ReadyPulseBase<super::pulse::Provider>,
) -> HostAdvertisement {
    let mut kind = conduit_time::tick_semantic_contract();
    kind.limits.max_queue_bytes = 4096;
    let capabilities = vec![BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: "test/environment/timer".into(),
            execution_profile_id: "test/environment/timer@1".into(),
            implementation_id: "test/environment/timer@1".into(),
            artifact_id: "test/environment/timer@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                target_kind: Some(kind.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: 1024,
                maximum_output_bytes: 1024,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()];
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: super::pulse::identity().host_id,
        boot_id: super::pulse::identity().boot_id,
        offer_generation: OfferGeneration(1),
        profile: "test/environment/appliance".into(),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    };
    ready
        .append_to_advertisement(&mut host, &PulseReadContract::prepare().unwrap())
        .unwrap();
    software.append_to_advertisement(&mut host).unwrap();
    host
}

/// Host-side preparation of the exact authored Plan; no model provider is built.
pub fn native_image(
    plan: &Plan,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> conduit_embedded_build::GeneratedEmbeddedPlan {
    conduit_embedded_build::generate_embedded_plan_with_computations(
        &plan.fragments[0],
        lowered,
        conduit_embedded_build::EmbeddedImageBounds {
            maximum_nodes: 15,
            maximum_cords: 14,
            maximum_routes: 15,
            maximum_route_targets: 14,
            maximum_host_calls: 15,
            maximum_resources: 1,
            maximum_sign_expectations: 128,
            maximum_configuration_entries: 16,
            maximum_ports_per_node:
                conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 14,
            maximum_cord_value_bytes: 32768,
            maximum_sign_items: 128,
            maximum_sign_bytes: 32768,
        },
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
    )
    .unwrap()
}
