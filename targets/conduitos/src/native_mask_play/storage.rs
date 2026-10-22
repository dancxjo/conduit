//! Storage inspection and profile custody, separate from renderer/Show authority.
use super::*;
use conduit_kernel::ValueStorage;

/// Allocator-observed value backing and inline objects, not whole-system peak.
/// Face/Plan heaps, compositor surfaces and serializer temporaries are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct NativeMaskStorageBudget {
    pub maximum_value_bytes: u32,
    pub value_slots: usize,
    pub reserved_value_payload_bytes: usize,
    pub reserved_value_storage_bytes: usize,
    pub live_value_byte_capacity: u32,
    pub scheduler_inline_bytes: usize,
    pub prepared_inline_bytes: usize,
}
pub(super) fn admit_budget(
    scheduler: &Scheduler,
    profile: NativeMaskStorageProfile,
) -> Result<NativeMaskStorageBudget, NativeMaskPlayError> {
    let values = scheduler.values();
    let (value_slots, payload) = values.allocation_capacities();
    let budget = NativeMaskStorageBudget {
        maximum_value_bytes: profile.bytes(),
        value_slots,
        reserved_value_payload_bytes: payload,
        reserved_value_storage_bytes: values.reserved_storage_bytes(),
        live_value_byte_capacity: values.byte_capacity(),
        scheduler_inline_bytes: core::mem::size_of::<Scheduler>(),
        prepared_inline_bytes: core::mem::size_of::<PreparedNativeMaskPlay>(),
    };
    if value_slots != VALUES
        || payload != VALUES * profile.bytes() as usize
        || budget.live_value_byte_capacity
            > profile.bytes() * 6 + 2 * INTERACTION_VALUE_BYTES as u32
    {
        return Err(NativeMaskPlayError::Pressure);
    }
    Ok(budget)
}

pub(super) fn validate_profile(
    planned: &PlannedMaskPlot,
    profile: NativeMaskStorageProfile,
) -> Result<(), NativeMaskPlayError> {
    if !conduit_core::verify_plan(&planned.plan) || planned.plan.fragments.len() != 1 {
        return Err(NativeMaskPlayError::Plan);
    }
    let fragment = &planned.plan.fragments[0];
    for placement in &fragment.placements {
        let expected = if placement.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND {
            match profile {
                NativeMaskStorageProfile::Standard => "conduitos/bounded-interaction@1",
                NativeMaskStorageProfile::Collection => {
                    "conduitos/bounded-collection-interaction@1"
                }
            }
        } else {
            profile.execution_profile()
        };
        if placement.execution_profile_id.as_str() != expected {
            return Err(NativeMaskPlayError::Plan);
        }
    }
    for (name, direction, bytes) in [
        ("face", conduit_core::PortDirection::Input, profile.bytes()),
        ("show", conduit_core::PortDirection::Output, profile.bytes()),
        (
            "interaction",
            conduit_core::PortDirection::Output,
            INTERACTION_VALUE_BYTES as u32,
        ),
    ] {
        let mut matching = fragment
            .fore_ports
            .iter()
            .filter(|p| p.front_port_id.as_str() == name && p.direction == direction);
        if !matching
            .next()
            .is_some_and(|p| p.item_capacity == 1 && p.byte_capacity == bytes)
            || matching.next().is_some()
        {
            return Err(NativeMaskPlayError::Shape);
        }
    }
    Ok(())
}
