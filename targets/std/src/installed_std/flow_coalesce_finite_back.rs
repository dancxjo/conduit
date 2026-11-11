//! Finite coalescing uses the same reference-forwarding Back as open coalescing.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{FrontValueLocation, PlannedGear};
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_COALESCE_FINITE_IMPLEMENTATION,
    budget,
    prepare,
};
fn validate(gear: &PlannedGear) -> Result<u32, String> {
    let value = gear
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("in")))
        .ok_or("finite coalescer requires its exact input envelope")?;
    let offer = conduit_std_offers::flow_coalesce_finite_offer(&value.contract);
    if gear.kind_id != offer.kind_id
        || gear.kind_contract_revision != offer.kind_contract_revision
        || gear.capability_id != offer.capability_id
        || gear.execution_profile_id != offer.implementation.execution_profile_id
        || gear.implementation_id != offer.implementation.implementation_id
        || gear.artifact_id != offer.implementation.artifact_id
        || gear.inputs != offer.inputs
        || gear.outputs != offer.outputs
        || gear.semantic_contract != offer.semantic_contract
        || gear.limits != offer.limits
        || !gear.configuration.is_empty()
        || !gear.host_calls.is_empty()
        || gear.base.is_some()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.realization_properties.is_empty()
        || !gear.realization_characteristics.is_empty()
    {
        return Err("finite coalescer differs from its selected realization".into());
    }
    Ok(value.contract.maximum_bytes)
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        maximum_value_bytes: validate(gear)?,
        host_requests: 0,
        sign_items: 32,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(gear)?;
    Ok(InstalledBack::FlowCoalesceLatest(
        super::flow_pressure_backs::FlowPressureBack,
    ))
}
