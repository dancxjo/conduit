use conduit_core::*;
use conduit_environment_runtime::construction::ApplianceSource;
use conduit_pulse_capture::{installation::ReadyPulseBase, PulseReadContract};

pub fn advertisement(
    source: &ApplianceSource,
    ready: &ReadyPulseBase<super::pulse::Provider>,
) -> HostAdvertisement {
    let capabilities = source
        .expanded
        .gears
        .iter()
        .enumerate()
        .filter(|(_, gear)| gear.kind_id.as_str() != conduit_pulse_capture::PULSE_READ_KIND)
        .map(|(index, gear)| {
            let kind = Kind {
                kind_id: gear.kind_id.clone(),
                kind_contract_revision: gear.kind_contract_revision.clone(),
                startup_parameters: gear.startup_parameters.clone(),
                shorthand: gear.shorthand.clone(),
                inputs: gear.inputs.clone(),
                outputs: gear.outputs.clone(),
                configuration: gear.semantic_contract.configuration.clone(),
                semantic_laws: gear.semantic_contract.laws.clone(),
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 4096,
                },
            };
            let call = if gear.kind_id.as_str() == conduit_time::TICK_KIND {
                Some(WAIT_HOST_CALL_CONTRACT)
            } else if gear.configuration.iter().any(|c| c.key == "selector") {
                Some("conduit.host/structured-selector@1")
            } else {
                None
            };
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: format!("test/environment/cap/{index}").into(),
                    execution_profile_id: "test/environment/prepared@1".into(),
                    implementation_id: format!("test/environment/back/{index}").into(),
                    artifact_id: "test/environment/production-back-conformance@1".into(),
                    host_calls: call
                        .map(|contract| {
                            vec![HostCallRequirement {
                                contract_id: contract.into(),
                                target_kind: Some(gear.kind_id.clone()),
                                maximum_in_flight: 1,
                                maximum_input_bytes: 1024,
                                maximum_output_bytes: 1024,
                            }]
                        })
                        .unwrap_or_default(),
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect();
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
    host
}
