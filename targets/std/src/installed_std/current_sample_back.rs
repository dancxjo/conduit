//! Std identity and budget binding for the shared current-value sampler.

use super::back::{BackBudget, BackFactory, InstalledBack};
pub(super) use conduit_composite::CurrentSampleBack;
use conduit_core::{CheckedValueContract, FrontValueLocation, PlannedGear};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::CURRENT_SAMPLE_IMPLEMENTATION,
    budget,
    prepare,
};

fn exact_contracts(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract), String> {
    let contracts = placement.semantic_contract.value_contracts();
    let current = contracts
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("current")))
        .ok_or("current/sample placement has no exact current value contract")?;
    let trigger = contracts
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("trigger")))
        .ok_or("current/sample placement has no exact trigger value contract")?;
    let output = contracts
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("value")))
        .ok_or("current/sample placement has no exact output value contract")?;
    if current.contract != output.contract {
        return Err("current/sample current and output specializations differ".into());
    }
    Ok((&current.contract, &trigger.contract))
}

fn validate(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract), String> {
    let (value, trigger) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::current_sample_semantic_contract(value, trigger)
        .map_err(str::to_string)?;
    if value.maximum_bytes > conduit_std_offers::CURRENT_SAMPLE_MAXIMUM_VALUE_BYTES
        || trigger.maximum_bytes > conduit_std_offers::CURRENT_SAMPLE_MAXIMUM_TRIGGER_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::CURRENT_SAMPLE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::CURRENT_SAMPLE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::CURRENT_SAMPLE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::CURRENT_SAMPLE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::CURRENT_SAMPLE_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned current/sample identity differs from its exact specialization".into());
    }
    Ok((value, trigger))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (value, _) = validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: value.maximum_bytes,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: value.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let maximum = validate(placement)?.0.maximum_bytes;
    Ok(InstalledBack::CurrentSample(CurrentSampleBack::prepare(
        maximum,
    )))
}
