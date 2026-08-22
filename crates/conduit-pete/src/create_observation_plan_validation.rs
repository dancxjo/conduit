use crate::{
    CreateObservationChannel, CreateObservationEvidence, CREATE_DEVICE_RESOURCE,
    CREATE_OBSERVATION_RESOURCE, CREATE_UART_BASE_RESOURCE,
};
use conduit_core::{Plan, ResourceClassId, ResourcePoolId};

pub(super) fn validate_plan(
    plan: &Plan,
    channel: CreateObservationChannel,
    evidence: &CreateObservationEvidence,
) -> Result<(), &'static str> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == channel.implementation_id())
        .ok_or("Plan has no requested Create observation placement")?;
    if placement.host_id != evidence.host_id
        || placement.boot_id != evidence.boot_id
        || placement.offer_generation != evidence.offer_generation
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != channel.operation_id()
        || placement.host_operations[0].maximum_input_bytes != 0
        || placement.resources.len() != 3
    {
        return Err("Plan does not seal the exact Create observation contract");
    }
    for (class, pool) in [
        (CREATE_UART_BASE_RESOURCE, evidence.serial_base_id.as_str()),
        (CREATE_DEVICE_RESOURCE, evidence.robot_identity.as_str()),
        (
            CREATE_OBSERVATION_RESOURCE,
            evidence.session_resource_id.as_str(),
        ),
    ] {
        if !placement.resources.iter().any(|binding| {
            binding.class_id == ResourceClassId::from(class)
                && binding.pool_id == ResourcePoolId::from(pool)
                && binding.units == 1
        }) {
            return Err("Plan resource binding does not match observed Create identity");
        }
    }
    Ok(())
}
