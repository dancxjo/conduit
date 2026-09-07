//! Tick bytes are validated at the kernel value boundary; all outputs exist before Play.
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
#[cfg(test)]
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_time::{
    PulseObservationConfiguration, PulseObservationOperation, PULSE_OBSERVATION_ENCODED_LEN,
    TICK_ENCODED_LEN,
};

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
    let configuration = validate(placement)?;
    Ok(OperationBudget {
        value_items: configuration.maximum_pulses,
        value_bytes: u32::from(configuration.maximum_pulses) * PULSE_OBSERVATION_ENCODED_LEN as u32,
        host_requests: 0,
        sign_items: configuration.maximum_pulses * 8 + 16,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let configuration = validate(placement)?;
    let mut outputs = Vec::with_capacity(configuration.maximum_pulses.into());
    for sequence in 0..u32::from(configuration.maximum_pulses) {
        let observation = configuration
            .observe(sequence, sequence.into())
            .expect("admitted sequence");
        outputs.push(
            values
                .store(&conduit_time::encode_pulse_observation(observation))
                .map_err(|error| format!("store pulse observation: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::PulseObserve(
        PulseObservationOperation::from_prepared_outputs(configuration, outputs)
            .map_err(str::to_owned)?,
    ))
}

#[cfg(test)]
#[path = "pulse_observation_tests.rs"]
mod tests;
