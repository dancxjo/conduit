//! Tick bytes are validated at the kernel value boundary; output storage is transaction-local.
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::HostedValueStore;
#[cfg(test)]
use conduit_kernel::{
    Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef, ValueStorage,
};
use conduit_time::{PulseObservationConfiguration, PulseObservationOperation, TICK_ENCODED_LEN};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::PULSE_OBSERVE_IMPLEMENTATION,
    budget,
    prepare,
};

#[cfg(test)]
fn failure(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

fn validate(placement: &PlannedGear) -> Result<PulseObservationConfiguration, String> {
    let offer = conduit_std_offers::pulse_observe_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned pulse observation identity does not match installation".into());
    }
    PulseObservationConfiguration::parse(&placement.configuration)
        .map_err(|error| format!("invalid pulse observation configuration: {error:?}"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let configuration = validate(placement)?;
    Ok(InstalledOperation::PulseObserve(
        PulseObservationOperation::new(configuration),
    ))
}

#[cfg(test)]
#[path = "pulse_observation_tests.rs"]
mod tests;
