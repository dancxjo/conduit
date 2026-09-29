//! Std installation of the shared bounded processing-time window operation.

use super::back::{BackBudget, BackFactory, InstalledBack};
use super::timing_configuration;
use conduit_core::{
    encode_monotonic_duration, CheckedValueContract, FrontValueLocation, PlannedGear,
};
use conduit_kernel::ValueStorage;

pub(super) type TimeWindowBack = conduit_time::ProcessingTimeWindowBack;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TIME_WINDOW_IMPLEMENTATION,
    budget,
    prepare,
};

fn exact_contracts(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract, u16), String> {
    let input = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/window placement has no exact input value contract")?;
    let output = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("window")))
        .ok_or("time/window placement has no exact output value contract")?;
    let maximum_items = placement
        .semantic_contract
        .laws
        .iter()
        .find_map(|law| match law {
            conduit_core::KindSemanticLaw::Terminal(
                conduit_core::KindTerminalBehavior::TumblingProcessingTimeWindow { maximum_items },
            ) => Some(*maximum_items),
            _ => None,
        })
        .ok_or("time/window placement has no exact window law")?;
    Ok((&input.contract, &output.contract, maximum_items))
}

fn validate(
    placement: &PlannedGear,
) -> Result<(&CheckedValueContract, &CheckedValueContract, u16), String> {
    let (value, window, maximum_items) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::time_window_semantic_contract(value, maximum_items)
        .map_err(str::to_string)?;
    let offer =
        conduit_std_offers::time_window_offer(value, maximum_items).map_err(str::to_string)?;
    if window.maximum_bytes as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::TIME_WINDOW_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TIME_WINDOW_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TIME_WINDOW_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::TIME_WINDOW_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TIME_WINDOW_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned time/window identity differs from its exact specialization".into());
    }
    Ok((value, window, maximum_items))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, window, _) = validate(placement)?;
    let value_bytes = window
        .maximum_bytes
        .checked_add(8)
        .ok_or("time/window prepared value budget overflows")?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes,
        host_requests: 1,
        sign_items: 128,
        maximum_value_bytes: window.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (value, window, maximum_items) = validate(placement)?;
    let duration_ms = timing_configuration::parse_window(placement)?;
    let duration = values
        .store(&encode_monotonic_duration(duration_ms))
        .map_err(|error| format!("store admitted time/window duration: {error:?}"))?;
    let operation = TimeWindowBack::prepare(duration, value, window.maximum_bytes, maximum_items)?;
    Ok(InstalledBack::TimeWindow(Box::new(operation)))
}
