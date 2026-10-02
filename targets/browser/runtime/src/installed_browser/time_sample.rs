//! Dynamic browser installation of an exact checked `time/sample<T>`.

use super::factory::BrowserInstallation;
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, FrontValueLocation, ImplementationId, PlannedGear,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-time-sample@1";
const PROFILE: &str = "browser/time-sample-kernel@1";
const ARTIFACT: &str = "conduit-browser-runtime/time-sample@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};

pub(crate) fn offer_for_expanded(
    gear: &conduit_plot::CheckedGear,
) -> Result<CapabilityOffer, String> {
    offer(exact_value(&gear.semantic_contract)?)
}

fn exact_value(
    semantic: &conduit_core::KindSemanticContract,
) -> Result<&CheckedValueContract, String> {
    let input = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/sample browser placement has no exact input contract")?;
    let output = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("sample")))
        .ok_or("time/sample browser placement has no exact output contract")?;
    if input.contract != output.contract {
        return Err("time/sample browser input and output contracts differ".into());
    }
    Ok(&input.contract)
}

fn offer(value: &CheckedValueContract) -> Result<CapabilityOffer, String> {
    let contract =
        conduit_semantic_catalog::time_sample_semantic_contract(value).map_err(str::to_string)?;
    let target = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("browser/{}", target.as_str())),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    let value = exact_value(&placement.semantic_contract)?;
    let exact = offer(value)?;
    if placement.kind_id != exact.kind_id
        || placement.kind_contract_revision != exact.kind_contract_revision
        || placement.capability_id != exact.capability_id
        || placement.execution_profile_id != exact.implementation.execution_profile_id
        || placement.implementation_id != exact.implementation.implementation_id
        || placement.artifact_id != exact.implementation.artifact_id
        || placement.inputs != exact.inputs
        || placement.outputs != exact.outputs
        || placement.semantic_contract != exact.semantic_contract
        || placement.limits != exact.limits
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned time/sample differs from its exact browser realization".into());
    }
    Ok(BrowserBack::installed_step(
        conduit_time::CadenceSampleBack::prepare(value.maximum_bytes as usize)?,
    ))
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic time/sample offers must be derived from checked source")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_specialization_produces_one_exact_browser_offer() {
        let value =
            CheckedValueContract::new(conduit_core::kind_id("value/text"), 73, vec![]).unwrap();
        let kind = conduit_semantic_catalog::time_sample_semantic_contract(&value).unwrap();
        let gear = conduit_plot::checked_gear_from_parts! {
            gear_id: conduit_core::GearId::from("sample"),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            semantic_contract: kind.semantic_contract(),
            terminal_transductions: Vec::new(),
            resource_ports: Vec::new(),
            configuration: Vec::new(),
            pool_references: Vec::new(),
        };
        let offer = offer_for_expanded(&gear).unwrap();
        assert_eq!(offer.kind_id, kind.kind_id);
        assert_eq!(offer.semantic_contract, kind.semantic_contract());
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            IMPLEMENTATION
        );
        assert!(offer.host_calls.is_empty());
        assert!(offer.resource_requirements.is_empty());
    }
}
