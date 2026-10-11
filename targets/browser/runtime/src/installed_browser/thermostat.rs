//! Browser installation of the portable domain-owned Thermostat transition.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, PlannedGear,
};
use conduit_kernel::HostedValueStore;

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-thermostat-combine@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_thermostat_plot::thermostat_kind(),
        Back {
            capability_id: CapabilityId::from("browser-thermostat-combine-v1"),
            execution_profile_id: ExecutionProfileId::from(
                "conduit.browser/thermostat-combine-bounded@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/thermostat-combine@1"),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate(placement)?;
    Ok(BrowserBack::installed_step(
        conduit_thermostat_plot::ThermostatBack::new(),
    ))
}

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected = offer();
    validate_placement(placement, &expected)?;
    if placement.capability_id != expected.capability_id
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned thermostat differs from exact installed browser Back".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_browser_back_preserves_exact_domain_contract_without_host_calls() {
        let semantic = conduit_thermostat_plot::thermostat_kind();
        let offered = offer();
        assert_eq!(offered.kind_id, semantic.kind_id);
        assert_eq!(
            offered.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offered.inputs, semantic.inputs);
        assert_eq!(offered.outputs, semantic.outputs);
        assert_eq!(offered.limits, semantic.limits);
        assert!(offered.host_calls.is_empty());
        assert!(offered.resource_requirements.is_empty());
        assert!(super::super::factory(&IMPLEMENTATION.into()).is_some());
    }
}
