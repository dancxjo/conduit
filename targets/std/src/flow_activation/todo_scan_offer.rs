//! Exact std Todo scan offer construction before live Host advertisement.
//!
//! A scan Kind contains its initial Form, so the offer is constructed from
//! owner-validated Todo state. The live std inventory must not advertise this
//! Back until the installed Body activation Play and Sign route are accepted.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer,
    CheckedValueContract, ExecutionProfileId, ImplementationId, PlannedGear, PlannedScanActivation,
};
use conduit_todo_plot::{
    TodoState, COMMAND_MAX_BYTES, STATE_MAX_BYTES, TODO_COMMAND_INFO_ID, TODO_STATE_INFO_ID,
};

pub const MAXIMUM_ITEMS: u16 = 64;
const EXECUTION_PROFILE: &str = "conduit.std/todo-scan-kernel@1";
const IMPLEMENTATION: &str = "std/kernel-todo-scan@1";
const ARTIFACT: &str = "conduit-std-host/todo-scan@1";

/// Build the exact production Back offer for one canonical initial Todo Form.
/// This is selectable by the planner when explicitly supplied; it is not in
/// the live Host advertisement while activation Play is refused.
pub fn todo_scan_offer(initial: &TodoState, maximum_items: u16) -> Result<CapabilityOffer, String> {
    if maximum_items == 0 || maximum_items > MAXIMUM_ITEMS {
        return Err("std Todo scan admits 1..=64 commands per Play".into());
    }
    let initial = initial
        .encode_info()
        .map_err(|error| format!("invalid initial Todo Form: {error:?}"))?;
    let item = CheckedValueContract::new(
        kind_id(TODO_COMMAND_INFO_ID),
        COMMAND_MAX_BYTES as u32,
        vec![],
    )
    .map_err(|error| format!("Todo command contract: {error:?}"))?;
    let accumulator =
        CheckedValueContract::new(kind_id(TODO_STATE_INFO_ID), STATE_MAX_BYTES as u32, vec![])
            .map_err(|error| format!("Todo state contract: {error:?}"))?;
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
            capability_id: CapabilityId::from("std/todo-scan@1"),
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
pub(crate) fn validate_planned_todo_scan(
    placement: &PlannedGear,
    planned: &PlannedScanActivation,
) -> Result<(), String> {
    let initial = TodoState::decode_info(&planned.initial_accumulator)
        .map_err(|_| "planned Todo scan initial Form is invalid")?;
    if initial
        .encode_info()
        .map_err(|_| "planned Todo scan initial Form is invalid")?
        != planned.initial_accumulator
    {
        return Err("planned Todo scan initial Form is not canonical".into());
    }
    let expected = todo_scan_offer(&initial, planned.limits.maximum_items)?;
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
        return Err("planned Todo scan differs from exact std production offer".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "todo_scan_offer_tests.rs"]
mod tests;
