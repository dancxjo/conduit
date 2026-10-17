//! Exact bounded Thermostat scan offer bound to a canonical initial Form.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer,
    CheckedValueContract, ExecutionProfileId, ImplementationId, PlannedGear, PlannedScanActivation,
};
use conduit_thermostat_plot::{
    ThermostatState, COMMAND_BYTES, COMMAND_KIND, STATE_BYTES, STATE_KIND,
};

pub const MAXIMUM_ITEMS: u16 = 256;
const EXECUTION_PROFILE: &str = "conduit.std/thermostat-scan-kernel@1";
const IMPLEMENTATION: &str = "std/kernel-thermostat-scan@1";
const ARTIFACT: &str = "conduit-std-host/thermostat-scan@1";

/// Select the ordinary Flow scan with an exact canonical Thermostat accumulator.
pub fn thermostat_scan_offer(
    initial: &ThermostatState,
    maximum_items: u16,
) -> Result<CapabilityOffer, String> {
    if maximum_items == 0 || maximum_items > MAXIMUM_ITEMS {
        return Err("std Thermostat scan admits 1..=256 commands per Play".into());
    }
    let initial = initial
        .encode_info()
        .map_err(|error| format!("invalid initial Thermostat Form: {error:?}"))?;
    let item = CheckedValueContract::new(kind_id(COMMAND_KIND), COMMAND_BYTES as u32, vec![])
        .map_err(|error| format!("Thermostat command contract: {error:?}"))?;
    let accumulator = CheckedValueContract::new(kind_id(STATE_KIND), STATE_BYTES as u32, vec![])
        .map_err(|error| format!("Thermostat state contract: {error:?}"))?;
    let kind = conduit_semantic_catalog::flow_scan_semantic_contract(
        &item,
        &accumulator,
        &initial,
        None,
        maximum_items,
    )
    .map_err(str::to_string)?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from("std/thermostat-scan@1"),
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}

/// A prepared owner must name the exact production Back and semantic initial
/// Form. A proof offer with copied semantic fields is not an installed Back.
pub(crate) fn validate_planned_thermostat_scan(
    placement: &PlannedGear,
    planned: &PlannedScanActivation,
) -> Result<(), String> {
    let initial = ThermostatState::decode_info(&planned.initial_accumulator)
        .map_err(|_| "planned Thermostat scan initial Form is invalid")?;
    if initial
        .encode_info()
        .map_err(|_| "planned Thermostat scan initial Form is invalid")?
        != planned.initial_accumulator
    {
        return Err("planned Thermostat scan initial Form is not canonical".into());
    }
    let expected = thermostat_scan_offer(&initial, planned.limits.maximum_items)?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Thermostat scan differs from exact std production offer".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "thermostat_scan_offer_tests.rs"]
pub(crate) mod tests;
