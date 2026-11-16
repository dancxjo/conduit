//! Exact typed standard profile using the shared allocation-free temporal lift.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_SINGLETON_IMPLEMENTATION,
    budget,
    prepare,
};
fn budget(p: &PlannedGear) -> Result<BackBudget, String> {
    let expected = conduit_std_offers::measurement_series_singleton_std_offer();
    if p.kind_id != expected.kind_id
        || p.kind_contract_revision != expected.kind_contract_revision
        || p.semantic_contract != expected.semantic_contract
        || p.capability_id != expected.capability_id
        || p.execution_profile_id != expected.implementation.execution_profile_id
        || p.implementation_id != expected.implementation.implementation_id
        || p.artifact_id != expected.implementation.artifact_id
        || p.inputs != expected.inputs
        || p.outputs != expected.outputs
        || p.host_calls != expected.host_calls
        || p.limits != expected.limits
        || !p.configuration.is_empty()
    {
        return Err("singleton differs from exact installed series profile".into());
    }
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: expected.limits.max_queue_bytes,
    })
}
fn prepare(
    p: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(p)?;
    Ok(InstalledBack::FlowSingleton(
        conduit_data::FlowSingletonStep,
    ))
}
