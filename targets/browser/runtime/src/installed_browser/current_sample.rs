//! Dynamic browser installation of an exact checked `current/sample<T>`.

use super::factory::BrowserInstallation;
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, CheckedValueContract,
    ExecutionProfileId, FrontValueLocation, ImplementationId, PlannedGear,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-current-sample@1";
const PROFILE: &str = "browser/current-sample-kernel@1";
const ARTIFACT: &str = "conduit-browser-runtime/current-sample@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};

pub(crate) fn offer_for_expanded(
    gear: &conduit_plot::CheckedGear,
) -> Result<CapabilityOffer, String> {
    let (value, trigger) = exact_value(&gear.semantic_contract)?;
    offer(value, trigger)
}

fn exact_value(
    semantic: &conduit_core::KindSemanticContract,
) -> Result<(&CheckedValueContract, &CheckedValueContract), String> {
    let input = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("current")))
        .ok_or("current/sample browser placement has no exact input contract")?;
    let output = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("value")))
        .ok_or("current/sample browser placement has no exact output contract")?;
    if input.contract != output.contract {
        return Err("current/sample browser input and output contracts differ".into());
    }
    let trigger = semantic
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("trigger")))
        .ok_or("current/sample trigger contract is absent")?;
    Ok((&input.contract, &trigger.contract))
}

fn offer(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<CapabilityOffer, String> {
    if value.maximum_bytes > super::MAXIMUM_BROWSER_VALUE_BYTES as u32
        || trigger.maximum_bytes > super::MAXIMUM_BROWSER_VALUE_BYTES as u32
    {
        return Err(
            "browser current/sample specialization exceeds the prepared-value byte bound".into(),
        );
    }
    let contract = conduit_semantic_catalog::current_sample_semantic_contract(value, trigger)
        .map_err(str::to_string)?;
    // The generic Kind ID is shared by all exact value/trigger contracts.
    // Capability identity must preserve both specializations across bundles.
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"conduit.browser/current-sample-specialization@1\0");
    for contract in [value, trigger] {
        let bytes = contract.identity_bytes();
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    let identity = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("browser/current-sample/{identity}")),
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
    let (value, trigger) = exact_value(&placement.semantic_contract)?;
    let exact = offer(value, trigger)?;
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
        return Err("planned current/sample differs from its exact browser realization".into());
    }
    Ok(BrowserBack::installed_step(
        conduit_composite::CurrentSampleBack::prepare(value.maximum_bytes),
    ))
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic current/sample offers must be derived from checked source")
}

pub(crate) fn install_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let frequency = CheckedValueContract::new(
        conduit_core::kind_id(
            &conduit_core::quantity_role_info_id(
                conduit_core::Unit::Hertz.family(),
                conduit_core::QuantityRole::Linear,
            )
            .map_err(|error| format!("frequency role: {error:?}"))?,
        ),
        conduit_core::QUANTITY_ENCODED_LEN as u32,
        vec![],
    )
    .map_err(|error| format!("frequency contract: {error:?}"))?;
    let tick = CheckedValueContract::new(
        conduit_core::kind_id(conduit_time::TICK_VALUE_KIND),
        conduit_time::TICK_ENCODED_LEN,
        vec![],
    )
    .map_err(|error| format!("tick contract: {error:?}"))?;
    conduit_semantic_catalog::install_current_sample_kind(&frequency, &tick, startup, profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::kind_id;

    #[test]
    fn capability_identity_separates_value_and_trigger_specializations() {
        let byte = CheckedValueContract::new(kind_id("value/u8"), 1, vec![]).unwrap();
        let word = CheckedValueContract::new(kind_id("value/u16"), 2, vec![]).unwrap();
        let baseline = offer(&byte, &byte).unwrap();
        assert_eq!(
            baseline.capability_id,
            offer(&byte, &byte).unwrap().capability_id
        );
        assert_ne!(
            baseline.capability_id,
            offer(&word, &byte).unwrap().capability_id
        );
        assert_ne!(
            baseline.capability_id,
            offer(&byte, &word).unwrap().capability_id
        );
        let wider = CheckedValueContract::new(kind_id("value/u8"), 2, vec![]).unwrap();
        assert_ne!(
            baseline.capability_id,
            offer(&wider, &byte).unwrap().capability_id
        );
        assert_ne!(
            baseline.capability_id,
            offer(&byte, &wider).unwrap().capability_id
        );
    }
}
