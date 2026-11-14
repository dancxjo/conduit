//! Trusted structural fingerprints of the ordinary assigned record layout.
use crate::{
    encode_assigned_kernel_plan, encode_assigned_native_plan, GeneratedEmbeddedPlan,
    GenerationError,
};
use conduit_core::{AssignedPlanMaxima, ASSIGNED_CONFIGURATION, ASSIGNED_SIGN, ASSIGNED_TERMINAL};
/// Resident structural inventory for a fixed-source assigned-kernel layout.
/// Configurations have their separate exact inventory. Sign and terminal IDs
/// depend on the current Plan/Boot and are not executable structural tables.
/// Only derive this from a validated generated image on the trusted Make side.
pub fn assigned_layout_requirements(
    plan: &GeneratedEmbeddedPlan,
    maxima: AssignedPlanMaxima,
) -> Result<Vec<conduit_core::AssignedRecordRequirement>, GenerationError> {
    layout_records(
        encode_assigned_kernel_plan(plan, maxima)?,
        conduit_core::ASSIGNED_KERNEL_PLAN_SCHEMA,
    )
}
pub fn assigned_native_layout_requirements(
    plan: &GeneratedEmbeddedPlan,
    maxima: AssignedPlanMaxima,
) -> Result<Vec<conduit_core::AssignedRecordRequirement>, GenerationError> {
    layout_records(
        encode_assigned_native_plan(plan, maxima)?,
        conduit_core::ASSIGNED_NATIVE_PLAN_SCHEMA,
    )
}
pub(crate) fn layout_records(
    bytes: Vec<u8>,
    schema: u16,
) -> Result<Vec<conduit_core::AssignedRecordRequirement>, GenerationError> {
    layout_records_profile(bytes, schema, None)
}
pub(crate) fn layout_records_profile(
    bytes: Vec<u8>,
    schema: u16,
    profile: Option<conduit_core::AssignedLineForeProfile<'_>>,
) -> Result<Vec<conduit_core::AssignedRecordRequirement>, GenerationError> {
    conduit_core::AssignedPlanRecords::new(&bytes)
        .map_err(|_| GenerationError::InvalidFragment)?
        .filter_map(|record| match record {
            Ok(record)
                if !matches!(
                    record.tag,
                    ASSIGNED_CONFIGURATION | ASSIGNED_SIGN | ASSIGNED_TERMINAL
                ) =>
            {
                Some(Ok(match profile {
                    Some(profile) => {
                        conduit_core::AssignedRecordRequirement::from_kernel_record_with_line_fore(
                            record, profile,
                        )
                    }
                    None => {
                        conduit_core::AssignedRecordRequirement::from_kernel_record(record, schema)
                    }
                }))
            }
            Ok(_) => None,
            Err(_) => Some(Err(GenerationError::InvalidFragment)),
        })
        .collect()
}

/// Resident-code projection uses the same structural inventory and schema.
pub fn assigned_resident_native_layout_requirements(
    plan: &GeneratedEmbeddedPlan,
    maxima: AssignedPlanMaxima,
    bindings: &[crate::BoundResidentExpression],
) -> Result<Vec<conduit_core::AssignedRecordRequirement>, GenerationError> {
    layout_records(
        crate::encode_assigned_resident_native_plan(plan, maxima, bindings)?,
        conduit_core::ASSIGNED_NATIVE_PLAN_SCHEMA,
    )
}
