//! Production std selection of the exact bounded Thermostat combine Back.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::THERMOSTAT_COMBINE_IMPLEMENTATION,
    budget,
    prepare,
};

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = conduit_thermostat_plot::thermostat_kind();
    if placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.execution_profile_id.as_str() != conduit_std_offers::THERMOSTAT_COMBINE_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::THERMOSTAT_COMBINE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::THERMOSTAT_COMBINE_ARTIFACT
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.semantic_contract != kind.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Thermostat combine differs from its exact bounded Back".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: (2 * conduit_thermostat_plot::STATE_BYTES
            + conduit_thermostat_plot::COMMAND_BYTES) as u32,
        maximum_value_bytes: conduit_thermostat_plot::STATE_BYTES as u32,
        host_requests: 0,
        sign_items: 3,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::ThermostatCombine(Box::new(
        conduit_thermostat_plot::ThermostatBack::new(),
    )))
}
