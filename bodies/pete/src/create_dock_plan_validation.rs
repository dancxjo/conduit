use crate::{
    CreateDockObservation, CREATE_DEVICE_RESOURCE, CREATE_DOCK_AUTHORITY, CREATE_DOCK_CAPABILITY,
    CREATE_DOCK_IMPLEMENTATION, CREATE_DOCK_OPERATION, CREATE_DOCK_PROFILE,
    CREATE_DOCK_REDUCED_SAFETY_AUTHORITY, CREATE_DOCK_REDUCED_SAFETY_PROFILE, CREATE_DOCK_RESOURCE,
    CREATE_UART_BASE_RESOURCE,
};
use conduit_core::{
    kind_id, port_id, verify_plan, ConfigurationValue, GearId, Plan, ResourceClassId,
    ResourcePoolId, BOOL_ENCODED_LEN, BOOL_INFO_ID, TIMER_RESOURCE_CLASS,
};

pub(super) struct ValidatedCreateDockPlan {
    pub timeout_ms: u32,
    pub authority_grant_id: String,
}

pub(super) fn validate_create_dock_plan(
    plan: &Plan,
    evidence: &CreateDockObservation,
) -> Result<ValidatedCreateDockPlan, &'static str> {
    if !verify_plan(plan) {
        return Err("Create dock Plan seal is invalid");
    }
    let placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    let connections = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    if placements.len() != 2 || connections.len() != 1 {
        return Err("Create dock Plan does not retain its exact portable graph");
    }
    let source = placements
        .iter()
        .find(|placement| placement.gear_id == GearId::from("seek_dock/request"))
        .ok_or("Create dock Plan has no exact request source")?;
    let dock_graph_placement = placements
        .iter()
        .find(|placement| placement.gear_id == GearId::from("seek_dock/dock"))
        .ok_or("Create dock Plan has no exact dock Gear")?;
    let initial_true = source
        .configuration
        .iter()
        .any(|entry| entry.key == "initial" && entry.value == ConfigurationValue::Bool(true));
    let connection = connections[0];
    if source.kind_id.as_str() != conduit_semantic_catalog::STATE_TOGGLE_KIND
        || source.implementation_id.as_str() != conduit_std_offers::STATE_TOGGLE_IMPLEMENTATION
        || !initial_true
        || dock_graph_placement.kind_id.as_str() != conduit_semantic_catalog::ROBOTICS_DOCK_KIND
        || connection.source_placement_id != source.placement_id
        || connection.source_port_id != port_id("value")
        || connection.sink_placement_id != dock_graph_placement.placement_id
        || connection.sink_port_id != port_id("request")
        || connection.value_kind != kind_id(BOOL_INFO_ID)
        || connection.item_capacity != 1
        || connection.byte_capacity != BOOL_ENCODED_LEN as u32
        || connection.selected_line.is_some()
        || !connection.admitted_lines.is_empty()
    {
        return Err("Create dock Plan portable graph does not match the kernel profile");
    }
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == CREATE_DOCK_IMPLEMENTATION)
        .ok_or("Plan has no Create dock placement")?;
    let (expected_profile, expected_authority) =
        if evidence.safety.has_complete_independent_envelope() {
            (CREATE_DOCK_PROFILE, CREATE_DOCK_AUTHORITY)
        } else {
            (
                CREATE_DOCK_REDUCED_SAFETY_PROFILE,
                CREATE_DOCK_REDUCED_SAFETY_AUTHORITY,
            )
        };
    if placement.host_id != evidence.host_id
        || placement.boot_id != evidence.boot_id
        || placement.offer_generation != evidence.offer_generation
        || placement.execution_profile_id.as_str() != expected_profile
        || placement.capability_id.as_str() != CREATE_DOCK_CAPABILITY
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != CREATE_DOCK_OPERATION
        || placement.host_operations[0].maximum_input_bytes != BOOL_ENCODED_LEN as u32
        || placement.host_operations[0].maximum_output_bytes != 0
        || placement.resources.len() != 4
        || placement.authority.len() != 1
    {
        return Err("Plan does not seal the exact Create dock contract");
    }
    for (class, pool) in [
        (CREATE_UART_BASE_RESOURCE, evidence.serial_base_id.as_str()),
        (CREATE_DEVICE_RESOURCE, evidence.robot_identity.as_str()),
        (CREATE_DOCK_RESOURCE, evidence.dock_resource_id.as_str()),
        (TIMER_RESOURCE_CLASS, evidence.timer_resource_id.as_str()),
    ] {
        if !placement.resources.iter().any(|binding| {
            binding.class_id == ResourceClassId::from(class)
                && binding.pool_id == ResourcePoolId::from(pool)
                && binding.units == 1
        }) {
            return Err("Plan resource binding does not match observed Create dock identity");
        }
    }
    let authority = &placement.authority[0];
    if authority.contract_id.as_str() != expected_authority
        || authority.host_operation_contract_id.as_str() != CREATE_DOCK_OPERATION
        || authority.subject_kind != kind_id(BOOL_INFO_ID)
        || authority.host_id != evidence.host_id
        || authority.boot_id != evidence.boot_id
        || authority.capability_id.as_str() != CREATE_DOCK_CAPABILITY
    {
        return Err("Plan authority does not match the Create dock realization");
    }
    let timeout = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "timeout-ms")
        .and_then(|entry| match entry.value {
            ConfigurationValue::U64(value) => u32::try_from(value).ok(),
            _ => None,
        })
        .ok_or("Plan has no exact Create dock timeout")?;
    if !(conduit_semantic_catalog::ROBOTICS_DOCK_MINIMUM_TIMEOUT_MS
        ..=conduit_semantic_catalog::ROBOTICS_DOCK_MAXIMUM_TIMEOUT_MS)
        .contains(&u64::from(timeout))
    {
        return Err("Plan Create dock timeout is outside the realization");
    }
    Ok(ValidatedCreateDockPlan {
        timeout_ms: timeout,
        authority_grant_id: authority.grant_id.as_str().to_string(),
    })
}
