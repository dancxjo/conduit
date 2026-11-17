//! Borrowed requested-allocation accounting for the common local materializer.
//! This is storage shape, not Plan verification or allocator confinement.
use super::*;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentLoweringLayout {
    pub retained_owned_bytes: usize,
    pub refusal_owned_bytes: usize,
    pub nodes: usize,
    pub cords: usize,
    pub fore_ports: usize,
    pub route_targets: usize,
    pub queue_slots: u16,
    pub value_bytes: u32,
}

struct Bytes(usize);
impl Bytes {
    fn add(&mut self, n: usize) -> Result<(), LoweringError> {
        self.0 = self
            .0
            .checked_add(n)
            .ok_or(LoweringError::CapacityOverflow)?;
        Ok(())
    }
    fn table<T>(&mut self, n: usize) -> Result<(), LoweringError> {
        let bytes = n
            .checked_mul(size_of::<T>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(LoweringError::CapacityOverflow)?;
        self.add(bytes)
    }
}

/// No heap allocation, validation bypass, or borrowed status-as-authority.
/// Rich State/fusion/pool/remote/resource-transfer/terminal-profile layouts
/// remain available through the ordinary entrances but have no funded receipt
/// here until their nested owners are accounted independently.
pub fn fragment_lowering_layout(
    fragment: &PlanFragment,
) -> Result<FragmentLoweringLayout, LoweringError> {
    if !fragment.states.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.execution_regions.is_empty()
        || fragment.placements.iter().any(|p| {
            !p.terminal_transductions.is_empty()
                || p.resources
                    .iter()
                    .any(|r| r.protected.is_some() || r.compute.is_some() || r.content.is_some())
        })
        || fragment.connections.iter().any(|c| {
            c.resource.is_some()
                || c.selected_line.is_some()
                || !c.admitted_lines.is_empty()
                || !fragment
                    .placements
                    .iter()
                    .any(|p| p.placement_id == c.source_placement_id)
                || !fragment
                    .placements
                    .iter()
                    .any(|p| p.placement_id == c.sink_placement_id)
        })
        || fragment.fore_ports.iter().any(|f| {
            f.selected_line.is_some()
                || f.value_contract
                    .as_ref()
                    .is_some_and(|c| !c.constraints.is_empty())
                || fragment
                    .placements
                    .iter()
                    .find(|p| p.placement_id == f.placement_id)
                    .is_none_or(|p| !p.realization_properties.is_empty())
        })
    {
        return Err(LoweringError::UnsupportedOwnedLayout);
    }
    let n = fragment.placements.len();
    let c = fragment
        .connections
        .len()
        .checked_add(fragment.fore_ports.len())
        .ok_or(LoweringError::CapacityOverflow)?;
    let f = fragment.fore_ports.len();
    let mut bytes = Bytes(0);
    bytes.table::<LoweredNode>(n)?;
    bytes.table::<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>(n)?;
    bytes.table::<(NodeId, PlacementId)>(n)?;
    bytes.table::<LoweredCord>(c)?;
    bytes.table::<(CordId, ConnectionId)>(c)?;
    // These exact-capacity vectors are retained even for a zero-length local table.
    bytes.table::<LoweredRemoteEndpoint>(fragment.connections.len())?;
    bytes.table::<LoweredResourceCord>(fragment.connections.len())?;
    bytes.table::<LoweredForePort>(f)?;
    bytes.table::<KernelForeEndpointIdentity>(f)?;
    bytes.table::<LoweredRoute>(c)?;
    bytes.table::<LoweredSign>(fragment.expected_sign.len())?;
    bytes.add(fragment.plan_id.as_str().len())?;
    bytes.add(fragment.fragment_id.as_str().len())?;
    let mut refusal = 0;
    for p in &fragment.placements {
        bytes.add(p.placement_id.as_str().len())?; // node
        bytes.add(p.placement_id.as_str().len())?; // identity map
        refusal = refusal.max(p.placement_id.as_str().len());
        for ports in [&p.inputs, &p.outputs] {
            bytes.table::<LoweredPort>(ports.len())?;
            bytes.table::<KernelPortIdentity>(ports.len())?;
            for port in ports {
                bytes.add(port.port_id.as_str().len())?;
                bytes.add(port.port_id.as_str().len())?;
                bytes.add(port.value_kind.as_str().len())?;
                if let Some(kind) = &port.abnormal_kind {
                    bytes.add(kind.as_str().len())?;
                }
                refusal = refusal.max(
                    p.placement_id
                        .as_str()
                        .len()
                        .checked_add(port.port_id.as_str().len())
                        .ok_or(LoweringError::CapacityOverflow)?,
                );
            }
        }
        bytes.table::<LoweredHostCall>(p.host_calls.len())?;
        bytes.table::<(NodeId, HostCallId, HostCallContractId)>(p.host_calls.len())?;
        for call in &p.host_calls {
            bytes.add(call.contract_id.as_str().len())?;
            bytes.add(call.contract_id.as_str().len())?;
            if let Some(kind) = &call.target_kind {
                bytes.add(kind.as_str().len())?;
            }
        }
        bytes.table::<LoweredResource>(p.resources.len())?;
        bytes.table::<(NodeId, ResourceId, PlanResourceBinding)>(p.resources.len())?;
        for resource in &p.resources {
            bytes.add(resource.pool_id.as_str().len())?;
            bytes.add(resource.class_id.as_str().len())?;
        }
    }
    let mut slots = 0u16;
    let mut values = 0u32;
    for connection in &fragment.connections {
        bytes.add(connection.connection_id.as_str().len())?;
        bytes.add(connection.connection_id.as_str().len())?;
        refusal = refusal.max(connection.connection_id.as_str().len());
        slots = slots
            .checked_add(connection.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        values = values
            .checked_add(connection.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    let targets = fragment
        .connections
        .len()
        .checked_add(
            fragment
                .fore_ports
                .iter()
                .filter(|f| f.direction == PortDirection::Output)
                .count(),
        )
        .ok_or(LoweringError::CapacityOverflow)?;
    bytes.table::<RouteTarget>(targets)?;
    for fore in &fragment.fore_ports {
        let id = 10usize
            .checked_add(fore.front_port_id.as_str().len())
            .and_then(|n| n.checked_add(fore.placement_id.as_str().len()))
            .and_then(|n| n.checked_add(fore.gear_port_id.as_str().len()))
            .ok_or(LoweringError::CapacityOverflow)?;
        bytes.add(id)?;
        bytes.add(id)?; // cord and identity
        bytes.add(fore.front_port_id.as_str().len())?;
        bytes.add(fore.front_port_id.as_str().len())?;
        bytes.add(fore.value_kind.as_str().len())?;
        bytes.add(fore.value_kind.as_str().len())?;
        if let Some(kind) = &fore.abnormal_kind {
            bytes.add(kind.as_str().len())?;
        }
        if let Some(contract) = &fore.value_contract {
            bytes.add(contract.value_kind.as_str().len())?;
        }
        refusal = refusal.max(fore.front_port_id.as_str().len());
        slots = slots
            .checked_add(fore.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        values = values
            .checked_add(fore.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    for sign in &fragment.expected_sign {
        match sign {
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                bytes.add(id.as_str().len())?
            }
            ExpectedSign::ConnectionTerminal(id) => bytes.add(id.as_str().len())?,
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {}
        }
    }
    Ok(FragmentLoweringLayout {
        retained_owned_bytes: bytes.0,
        refusal_owned_bytes: refusal,
        nodes: n,
        cords: c,
        fore_ports: f,
        route_targets: targets,
        queue_slots: slots,
        value_bytes: values,
    })
}
