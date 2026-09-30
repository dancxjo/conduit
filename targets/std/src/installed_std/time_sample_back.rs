//! Std installation of the shared exact cadence sampler.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{CheckedValueContract, FrontValueLocation, PlannedGear};
use conduit_kernel::CanonicalValue;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) type TimeSampleBack = conduit_time::CadenceSampleBack;

fn exact_value_contract(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let input = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/sample placement has no exact input value contract")?;
    let output = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("sample")))
        .ok_or("time/sample placement has no exact output value contract")?;
    if input.contract != output.contract {
        return Err("time/sample input and output specializations differ".into());
    }
    Ok(&input.contract)
}

fn validate(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let value = exact_value_contract(placement)?;
    let expected =
        conduit_semantic_catalog::time_sample_semantic_contract(value).map_err(str::to_string)?;
    if value.maximum_bytes > conduit_std_offers::TIME_SAMPLE_MAXIMUM_VALUE_BYTES
        || value.maximum_bytes as usize > CanonicalValue::MAXIMUM_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::TIME_SAMPLE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TIME_SAMPLE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TIME_SAMPLE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TIME_SAMPLE_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned time/sample identity differs from its exact specialization".into());
    }
    Ok(value)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: value.maximum_bytes,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: value.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let maximum = validate(placement)?.maximum_bytes as usize;
    Ok(InstalledBack::TimeSample(TimeSampleBack::prepare(maximum)?))
}
