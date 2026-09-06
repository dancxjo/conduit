//! Installed lifecycle for one finite pressed-button timing attempt.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{encode_monotonic_duration, ConfigurationValue, PlannedGear};
use conduit_kernel::ValueStorage;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_time::TimedButtonAttemptOperation;

fn configuration(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("pressed-button attempt lacks {key}"))
}

fn validate(placement: &PlannedGear) -> Result<(u64, u64, u64), String> {
    let offer = conduit_std_offers::timed_button_attempt_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.authority.is_empty()
    {
        return Err("planned pressed-button attempt differs from installed realization".into());
    }
    for class in [
        conduit_core::TIMER_RESOURCE_CLASS,
        conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
    ] {
        if !placement.resources.iter().any(|resource| {
            resource.class_id.as_str() == class
                && resource.units == 1
                && resource.protected.is_none()
                && resource.compute.is_none()
        }) {
            return Err(format!("pressed-button attempt lacks admitted {class}"));
        }
    }
    let presses = configuration(placement, "maximum-presses")?;
    let transitions = configuration(placement, "maximum-transitions")?;
    let timeout = configuration(placement, "timeout-ms")?;
    if !(2..=conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u64).contains(&presses)
        || !(presses..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TRANSITIONS).contains(&transitions)
        || !(1..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TIMEOUT_MS).contains(&timeout)
    {
        return Err("pressed-button attempt configuration is outside reviewed bounds".into());
    }
    Ok((presses, transitions, timeout))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let (_, transitions, _) = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(transitions.saturating_mul(3) + 1)
            .map_err(|_| "pressed-button value budget overflow")?,
        value_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            .saturating_mul(transitions as u32 + 1),
        host_requests: usize::try_from(transitions.saturating_mul(2))
            .map_err(|_| "pressed-button request budget overflow")?,
        sign_items: u16::try_from(transitions.saturating_mul(12))
            .map_err(|_| "pressed-button sign budget overflow")?,
        maximum_value_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    let (_, transitions, timeout) = validate(placement)?;
    let durations = (0..transitions)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(timeout))
                .map_err(|error| format!("store pressed-button deadline: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TimedButtonAttempt(
        TimedButtonAttemptOperation::from_prepared_durations(durations, transitions),
    ))
}

pub(super) fn host_maximum(placement: &PlannedGear) -> Result<usize, String> {
    usize::try_from(validate(placement)?.0)
        .map_err(|_| "pressed-button maximum does not fit this Host".into())
}

pub(super) fn refusal_detail(refusal: super::timed_button_attempt_host::Refusal) -> u16 {
    use super::timed_button_attempt_host::Refusal::*;
    match refusal {
        MalformedTransition => 1,
        TooManyEvents => 2,
        ClockRegressed => 3,
    }
}
