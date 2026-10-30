use conduit_core::*;
use conduit_environment_runtime::construction::ApplianceSource;
use conduit_pulse_capture::{installation::ReadyPulseBase, PulseReadContract};

pub fn advertisement(
    _source: &ApplianceSource,
    software: &mut conduit_environment_runtime::appliance::software::ReadySoftwareBacks,
    ready: &ReadyPulseBase<super::pulse::Provider>,
) -> HostAdvertisement {
    let mut host = native_advertisement(ready);
    software.append_to_advertisement(&mut host).unwrap();
    host
}

/// CPU native/timer fixtures only. No portable software owner is synthesized.
pub fn native_advertisement(ready: &ReadyPulseBase<super::pulse::Provider>) -> HostAdvertisement {
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
        declared_name: None,
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
    host
}
