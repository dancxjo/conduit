//! Browser installation of the shared recurring pulse-observation operation.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};

const PROFILE: &str = "browser/pulse-observe-ordered-64@1";
const IMPLEMENTATION: &str = "browser/kernel-pulse-observe@1";
const ARTIFACT: &str = "conduit-browser-runtime/pulse-observe@1";

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_time::pulse_observe_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("pulse-observe"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserBack, String> {
    let installed = offer();
    super::factory::validate_placement(placement, &installed)?;
    if !placement.resources.is_empty() || !placement.authority.is_empty() {
        return Err("pulse observation admission differs from installation".into());
    }
    let configuration =
        conduit_time::PulseObservationConfiguration::parse(&placement.configuration)
            .map_err(|error| format!("invalid pulse observation configuration: {error:?}"))?;
    let operation = conduit_time::PulseObservationBack::new(configuration);
    Ok(super::BrowserBack::installed_step(operation))
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
    use conduit_kernel::HostedValueStore;

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
            base: None,
            realization_characteristics: vec![],
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            host_calls: offer.host_calls,
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }
    }

    #[test]
    fn browser_installs_shared_observer_with_all_outputs_admitted_before_play() {
        let mut values = HostedValueStore::new(64, 8, 512).unwrap();
        let capacity = values.allocation_capacities();
        let _back = prepare(&placement(), &mut values).unwrap();
        assert_eq!(values.allocation_capacities(), capacity);
    }

    #[test]
    fn browser_offer_and_preparation_fail_closed_on_identity_and_bounds() {
        let installed = offer();
        assert_eq!(installed.kind_id.as_str(), conduit_time::PULSE_OBSERVE_KIND);
        assert!(installed.host_calls.is_empty());
        assert!(installed.resource_requirements.is_empty());
        let mut values = HostedValueStore::new(64, 8, 512).unwrap();
        let mut wrong = placement();
        wrong.artifact_id = "foreign".into();
        assert!(prepare(&wrong, &mut values).is_err());
    }
}
