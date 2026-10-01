//! Browser installation of the shared one-shot semantic deadline.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserBack, BROWSER_TIMER_MAXIMUM_MILLIS};
use conduit_core::{
    encode_monotonic_duration, resource_requirement, ConfigurationValue, PlannedGear, QuantityUnit,
    TIMER_RESOURCE_CLASS,
};
use conduit_kernel::ValueStorage;

const IMPLEMENTATION: &str = "browser/kernel-time-deadline@1";
const ARTIFACT: &str = "conduit-browser-runtime/time-deadline@1";

pub(super) static TIME_DEADLINE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_deadline_contract(),
        conduit_semantic_catalog::TIME_DEADLINE_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![conduit_core::wait_host_call_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    let [entry] = placement.configuration.as_slice() else {
        return Err("time/deadline requires one exact duration".into());
    };
    let ("duration-ms", ConfigurationValue::Quantity(quantity)) = (&*entry.key, &entry.value)
    else {
        return Err("time/deadline duration is malformed".into());
    };
    let duration_ms: u64 = quantity
        .convert(QuantityUnit::Millisecond)
        .map_err(|_| "time/deadline duration is not milliseconds")?
        .value()
        .try_into()
        .map_err(|_| "time/deadline duration is negative")?;
    if duration_ms > BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("time/deadline exceeds the browser timer bound".into());
    }
    let duration = values
        .store(&encode_monotonic_duration(duration_ms))
        .map_err(|error| format!("store browser deadline: {error:?}"))?;
    let request = values
        .store(&[])
        .map_err(|error| format!("store cancellation request: {error:?}"))?;
    Ok(BrowserBack::installed_step(
        conduit_time::CancellationDeadlineBack::prepare(duration, request),
    ))
}
