//! Browser installation of the shared recurring pulse-observation operation.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, ImplementationId, ImplementationOffer,
};

const PROFILE: &str = "browser/pulse-observe-ordered-64@1";
const IMPLEMENTATION: &str = "browser/kernel-pulse-observe@1";
const ARTIFACT: &str = "conduit-browser-runtime/pulse-observe@1";

fn offer() -> CapabilityOffer {
    let contract = conduit_time::pulse_observe_kind_definition();
    CapabilityOffer {
        capability_id: CapabilityId::from("pulse-observe"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        startup_parameters: ["period-ms"]
            .into_iter()
            .map(|name| FaceStartupParameter {
                name: name.into(),
                value_type: "Count".into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::TICK_ENCODED_LEN,
        },
    }
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    let installed = offer();
    super::factory::validate_placement(placement, &installed)?;
    if !placement.resources.is_empty() || !placement.authority.is_empty() {
        return Err("pulse observation admission differs from installation".into());
    }
    let configuration =
        conduit_time::PulseObservationConfiguration::parse(&placement.configuration)
            .map_err(|error| format!("invalid pulse observation configuration: {error:?}"))?;
    let operation = conduit_time::PulseObservationOperation::new(configuration);
    Ok(super::BrowserOperation::installed(operation))
}

pub(super) static INSTALLATION: super::factory::BrowserInstallation =
    super::factory::BrowserInstallation {
        implementation_id: IMPLEMENTATION,
        offer,
        prepare,
        perform: None,
    };

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{ConfigurationEntry, ConfigurationValue, PlannedGear};
    use conduit_kernel::{HostedValueStore, Operation, OperationAction, PortId, ValueRef};

    fn placement() -> PlannedGear {
        let offer = offer();
        PlannedGear {
            placement_id: "browser-pulse-placement".into(),
            gear_id: "pulse".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![ConfigurationEntry {
                key: "period-ms".into(),
                value: ConfigurationValue::U64(240),
            }],
            host_id: "browser/pulse".into(),
            boot_id: "boot/pulse".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            realization_characteristics: vec![],
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            host_operations: offer.host_operations,
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }
    }

    #[test]
    fn browser_installs_shared_observer_with_all_outputs_admitted_before_play() {
        let mut values = HostedValueStore::new(64, 8, 512).unwrap();
        let mut operation = prepare(&placement(), &mut values).unwrap();
        let capacity = values.allocation_capacities();
        assert_eq!(operation.start(), OperationAction::Await);
        for sequence in 0..3 {
            let tick = conduit_time::encode_tick(sequence);
            let input = ValueRef {
                slot: 63,
                generation: 1,
                byte_len: tick.len() as u32,
            };
            let OperationAction::EmitCanonical { port, value } =
                operation.resume_value(PortId(0), input, &tick)
            else {
                panic!("browser observer must emit one exact pulse");
            };
            assert_eq!(port, PortId(0));
            let observed = conduit_time::decode_pulse_observation(value.as_slice()).unwrap();
            assert_eq!(
                (observed.sequence, observed.period_ms),
                (sequence as u32, 240)
            );
            assert_eq!(operation.advance(), OperationAction::Await);
        }
        assert_eq!(values.allocation_capacities(), capacity);
    }

    #[test]
    fn browser_offer_and_preparation_fail_closed_on_identity_and_bounds() {
        let installed = offer();
        assert_eq!(installed.kind_id.as_str(), conduit_time::PULSE_OBSERVE_KIND);
        assert!(installed.host_operations.is_empty());
        assert!(installed.resource_requirements.is_empty());
        let mut values = HostedValueStore::new(64, 8, 512).unwrap();
        let mut wrong = placement();
        wrong.artifact_id = "foreign".into();
        assert!(prepare(&wrong, &mut values).is_err());
    }
}
