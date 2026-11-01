//! Hosted registration of the same portable Tick-to-Empty Back used by MCUs.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TICK_TRIGGER_IMPLEMENTATION,
    budget,
    prepare,
};
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let kind = conduit_time::tick_trigger_semantic_contract();
    if gear.kind_id != kind.kind_id
        || gear.kind_contract_revision != kind.kind_contract_revision
        || gear.inputs != kind.inputs
        || gear.outputs != kind.outputs
        || gear.semantic_contract != kind.semantic_contract()
        || gear.implementation_id.as_str() != conduit_std_offers::TICK_TRIGGER_IMPLEMENTATION
        || gear.execution_profile_id.as_str() != conduit_std_offers::TICK_TRIGGER_IMPLEMENTATION
        || gear.artifact_id.as_str() != conduit_std_offers::TICK_TRIGGER_ARTIFACT
        || !gear.configuration.is_empty()
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
    {
        return Err("tick-trigger differs from exact installed portable Back".into());
    }
    Ok(BackBudget {
        value_items: 2,
        value_bytes: conduit_time::TICK_ENCODED_LEN,
        maximum_value_bytes: conduit_time::TICK_ENCODED_LEN,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::TickTrigger(
        conduit_time::TickTriggerBack::default(),
    ))
}
