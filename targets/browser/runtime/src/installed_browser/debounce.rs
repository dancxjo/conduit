//! Browser installation of the shared finite trailing-debounce operation.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserBack, BROWSER_TIMER_MAXIMUM_MILLIS};
use conduit_core::{
    encode_monotonic_duration, resource_requirement, ConfigurationValue, PlannedGear,
    TIMER_RESOURCE_CLASS,
};
use conduit_kernel::ValueStorage;

const IMPLEMENTATION: &str = "browser/kernel-time-debounce-bool@1";
const ARTIFACT: &str = "conduit-browser-runtime/installed-debounce@1";

pub(super) static TIME_DEBOUNCE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_debounce_contract(),
        conduit_semantic_catalog::TIME_DEBOUNCE_CONTRACT_REVISION,
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
    let duration = configured(placement, "duration-ms")?;
    if duration > BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("time/debounce duration-ms exceeds the browser timer bound".into());
    }
    if !placement.configuration.iter().any(|entry| matches!((&*entry.key, &entry.value), ("policy", ConfigurationValue::Text(value)) if value == conduit_semantic_catalog::TIME_POLICY_TRAILING)) {
        return Err("time/debounce supports only exact trailing policy".into());
    }
    let maximum_values = usize::try_from(configured(placement, "maximum-values")?)
        .map_err(|_| "time/debounce maximum-values does not fit")?;
    if maximum_values == 0
        || maximum_values > conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize
    {
        return Err("time/debounce maximum-values exceeds the browser bound".into());
    }
    let durations = (0..maximum_values)
        .map(|_| {
            values
                .store(&encode_monotonic_duration(duration))
                .map_err(debug_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let operation =
        conduit_time::TrailingDebounceBack::from_prepared_durations(durations, maximum_values)
            .map_err(debug_error)?;
    Ok(BrowserBack::installed_step(operation))
}

fn configured(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("time/debounce configuration '{key}' is missing"))
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
