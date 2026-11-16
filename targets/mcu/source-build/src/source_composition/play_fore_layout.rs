//! Fresh structural expectation from installed Source and initialized own offers.
//! No incoming assignment row supplies a placement or capability selector.
use super::*;

fn ordinal(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("line-layout-node")?
            .try_into()
            .map_err(|_| "line-layout-node")?,
    ))
}
fn capability(actual: &c::HostAdvertisement, node: u16) -> Result<&str, &'static str> {
    // The original installed native records already pin capability and Base
    // identities. Compact current truth retains their exact strings and Ready
    // provenance without retaining decoded capability schemas.
    let mut selected = None;
    for row in c::AssignedPlanRecords::new(source::sampling_layout::PROJECTION_TEMPLATE)
        .map_err(|_| "line-layout-template")?
    {
        let row = row.map_err(|_| "line-layout-template")?;
        if row.tag != c::ASSIGNED_HOST_CALL || ordinal(row.payload, 0)? != node {
            continue;
        }
        let binding = c::assigned_native_binding(row.payload)
            .map_err(|_| "line-layout-native")?
            .ok_or("line-layout-native")?;
        let mut bases = actual
            .bases
            .iter()
            .filter(|base| c::AssignedIdentity::from_text(base.base_id.as_str()) == binding.base);
        let base = bases.next().ok_or("line-layout-base-absent")?;
        if bases.next().is_some()
            || base.lifecycle != c::BaseLifecycle::Ready
            || c::AssignedIdentity::from_text(base.implementation_id.as_str())
                != binding.base_implementation
            || c::AssignedIdentity::from_text(base.mechanism_family.as_str()) != binding.mechanism
            || base.enforcement_class as u8 != binding.enforcement
        {
            return Err("line-layout-base-profile");
        }
        let mut capabilities = base
            .capability_ids
            .iter()
            .filter(|id| c::AssignedIdentity::from_text(id.as_str()) == binding.capability);
        let id = capabilities.next().ok_or("line-layout-capability-absent")?;
        if capabilities.next().is_some() {
            return Err("line-layout-capability-ambiguous");
        }
        if selected.is_some_and(|previous| previous != id.as_str()) {
            return Err("line-layout-native-capability-mismatch");
        }
        selected = Some(id.as_str());
    }
    // A resident-only boundary needs its initialized resident identity. Never
    // recover it by decoding offers or trusting an incoming assignment.
    selected.ok_or("line-layout-native-identity-absent")
}

pub fn placements(
    actual: &c::HostAdvertisement,
) -> Result<[heapless::String<64>; source::sampling_layout::FORE_PORTS], &'static str> {
    let mut result = core::array::from_fn(|_| heapless::String::new());
    for (slot, fore) in source::sampling_layout::FRESH_FORE.iter().enumerate() {
        let mut gears = source::sampling_layout::GEAR_IDS
            .iter()
            .filter(|(node, _)| *node == fore.local_node);
        let (_, gear) = gears.next().ok_or("line-layout-gear")?;
        if gears.next().is_some() {
            return Err("line-layout-gear-duplicate");
        }
        let offer = capability(actual, fore.local_node)?;
        let id = c::planned_placement_identity_bytes(
            source::sampling_layout::CHECKED_PLOT_ID,
            gear,
            actual.host_id.as_str(),
            offer,
        );
        result[slot]
            .push_str(core::str::from_utf8(&id).map_err(|_| "line-layout-placement")?)
            .map_err(|_| "line-layout-placement")?;
    }
    Ok(result)
}
