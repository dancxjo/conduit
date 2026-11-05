//! Lower only explicitly selected private-work reservations into Step fuel.
use super::LoweringError;
use conduit_core::{PlannedGear, COOPERATIVE_STEP_WORK_CLASS_ID};

pub(super) fn maximum_step_fuel(placement: &PlannedGear) -> Result<u16, LoweringError> {
    let mut fuel = 1usize
        .checked_add(placement.inputs.len())
        .and_then(|n| n.checked_add(placement.outputs.len()))
        .and_then(|n| n.checked_add(placement.host_calls.len()))
        .ok_or(LoweringError::CapacityOverflow)?;
    for binding in &placement.resources {
        if binding.class_id.as_str() != COOPERATIVE_STEP_WORK_CLASS_ID {
            continue;
        }
        // A work unit is an unprotected count, never a lane, content or device grant.
        if binding.units == 0
            || binding.protected.is_some()
            || binding.compute.is_some()
            || binding.content.is_some()
        {
            return Err(LoweringError::ResourceBindingInvalid(
                placement.placement_id.clone(),
            ));
        }
        fuel = fuel
            .checked_add(
                usize::try_from(binding.units).map_err(|_| LoweringError::CapacityOverflow)?,
            )
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    u16::try_from(fuel).map_err(|_| LoweringError::CapacityOverflow)
}
