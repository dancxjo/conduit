use crate::{
    CreateDriveObservation, CREATE_DEVICE_RESOURCE, CREATE_DRIVE_AUTHORITY,
    CREATE_DRIVE_CAPABILITY, CREATE_DRIVE_IMPLEMENTATION, CREATE_DRIVE_OPERATION,
    CREATE_DRIVE_PROFILE, CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
    CREATE_DRIVE_REDUCED_SAFETY_PROFILE, CREATE_DRIVE_RESOURCE, CREATE_UART_BASE_RESOURCE,
};
use conduit_core::{
    kind_id, ConfigurationValue, Plan, ResourceClassId, ResourcePoolId, SCALAR_INFO_ID,
};

pub(super) struct ValidatedCreateDrivePlan {
    pub ttl_ms: u32,
    pub authority_grant_id: String,
}

pub(super) fn validate_create_drive_plan(
    plan: &Plan,
    evidence: &CreateDriveObservation,
) -> Result<ValidatedCreateDrivePlan, &'static str> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == CREATE_DRIVE_IMPLEMENTATION)
        .ok_or("Plan has no Create drive placement")?;
    let (expected_profile, expected_authority) =
        if evidence.safety.has_complete_independent_envelope() {
            (CREATE_DRIVE_PROFILE, CREATE_DRIVE_AUTHORITY)
        } else {
            (
                CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
                CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
            )
        };
    if placement.host_id != evidence.host_id
        || placement.boot_id != evidence.boot_id
        || placement.offer_generation != evidence.offer_generation
        || placement.execution_profile_id.as_str() != expected_profile
        || placement.capability_id.as_str() != CREATE_DRIVE_CAPABILITY
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != CREATE_DRIVE_OPERATION
        || placement.host_operations[0].maximum_input_bytes
            != (2 * conduit_core::SCALAR_ENCODED_LEN) as u32
        || placement.host_operations[0].maximum_output_bytes != 0
        || placement.resources.len() != 3
        || placement.authority.len() != 1
    {
        return Err("Plan does not seal the exact Create drive contract");
    }
    for (class, pool) in [
        (CREATE_UART_BASE_RESOURCE, evidence.serial_base_id.as_str()),
        (CREATE_DEVICE_RESOURCE, evidence.robot_identity.as_str()),
        (CREATE_DRIVE_RESOURCE, evidence.drive_resource_id.as_str()),
    ] {
        if !placement.resources.iter().any(|binding| {
            binding.class_id == ResourceClassId::from(class)
                && binding.pool_id == ResourcePoolId::from(pool)
                && binding.units == 1
        }) {
            return Err("Plan resource binding does not match observed Create drive identity");
        }
    }
    let authority = &placement.authority[0];
    if authority.contract_id.as_str() != expected_authority
        || authority.host_operation_contract_id.as_str() != CREATE_DRIVE_OPERATION
        || authority.subject_kind != kind_id(SCALAR_INFO_ID)
        || authority.host_id != evidence.host_id
        || authority.boot_id != evidence.boot_id
        || authority.capability_id.as_str() != CREATE_DRIVE_CAPABILITY
    {
        return Err("Plan authority does not match the Create drive realization");
    }
    let ttl = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "ttl-ms")
        .and_then(|entry| match entry.value {
            ConfigurationValue::U64(value) => u32::try_from(value).ok(),
            _ => None,
        })
        .ok_or("Plan has no exact Create drive TTL")?;
    if !(crate::MINIMUM_MOTION_TTL_MS..=crate::MAXIMUM_MOTION_TTL_MS).contains(&ttl) {
        return Err("Plan Create drive TTL is outside the realization");
    }
    Ok(ValidatedCreateDrivePlan {
        ttl_ms: ttl,
        authority_grant_id: authority.grant_id.as_str().to_string(),
    })
}
