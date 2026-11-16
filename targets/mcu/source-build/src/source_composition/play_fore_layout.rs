//! Fresh structural expectation from installed Source and initialized own offers.
//! No incoming assignment row supplies a placement or capability selector.
use super::*;

fn identity(bytes: &[u8], offset: usize) -> Result<c::AssignedIdentity, &'static str> {
    Ok(c::AssignedIdentity(
        bytes
            .get(offset..offset + 16)
            .ok_or("line-layout-node")?
            .try_into()
            .map_err(|_| "line-layout-node")?,
    ))
}
fn ordinal(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("line-layout-node")?
            .try_into()
            .map_err(|_| "line-layout-node")?,
    ))
}
fn capability<'a>(
    actual: &'a c::HostAdvertisement,
    node: u16,
) -> Result<&'a c::CapabilityOffer, &'static str> {
    let bytes = source::sampling_layout::PROJECTION_TEMPLATE;
    let mut node_pin = None;
    for row in c::AssignedPlanRecords::new(bytes).map_err(|_| "line-layout-template")? {
        let row = row.map_err(|_| "line-layout-template")?;
        if row.tag == c::ASSIGNED_NODE && ordinal(row.payload, 0)? == node {
            if node_pin.replace(row.payload).is_some() {
                return Err("line-layout-node-duplicate");
            }
        }
    }
    let pin = node_pin.ok_or("line-layout-node-absent")?;
    let mut selected = None;
    for offer in &actual.capabilities {
        if c::AssignedIdentity::from_text(offer.kind_id.as_str()) != identity(pin, 4)?
            || c::AssignedIdentity::from_text(offer.implementation.implementation_id.as_str())
                != identity(pin, 20)?
            || c::AssignedIdentity::from_text(offer.implementation.artifact_id.as_str())
                != identity(pin, 36)?
        {
            continue;
        }
        let mut input = 0;
        let mut output = 0;
        let mut exact = true;
        for row in c::AssignedPlanRecords::new(bytes).map_err(|_| "line-layout-template")? {
            let row = row.map_err(|_| "line-layout-template")?;
            if row.tag != c::ASSIGNED_PORT || ordinal(row.payload, 0)? != node {
                continue;
            }
            let incoming = match row.payload.get(4) {
                Some(0) => true,
                Some(1) => false,
                _ => return Err("line-layout-port-direction"),
            };
            let port_id = identity(row.payload, 5)?;
            let value_kind = identity(row.payload, 21)?;
            let ports = if incoming {
                input += 1;
                &offer.inputs
            } else {
                output += 1;
                &offer.outputs
            };
            exact &= ports.iter().any(|port| {
                c::AssignedIdentity::from_text(port.port_id.as_str()) == port_id
                    && c::AssignedIdentity::from_text(port.value_kind.as_str()) == value_kind
            });
        }
        if !exact || input != offer.inputs.len() || output != offer.outputs.len() {
            continue;
        }
        if selected.replace(offer).is_some() {
            return Err("line-layout-capability-ambiguous");
        }
    }
    selected.ok_or("line-layout-capability-absent")
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
            offer.capability_id.as_str(),
        );
        result[slot]
            .push_str(core::str::from_utf8(&id).map_err(|_| "line-layout-placement")?)
            .map_err(|_| "line-layout-placement")?;
    }
    Ok(result)
}
