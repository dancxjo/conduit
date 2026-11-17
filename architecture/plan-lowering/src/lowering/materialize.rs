//! Fallible exact-capacity preparation and borrowed placement indexing.
use super::*;
use core::mem::size_of;

pub(crate) fn reserved<T>(capacity: usize) -> Result<Vec<T>, LoweringError> {
    capacity
        .checked_mul(size_of::<T>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or(LoweringError::CapacityOverflow)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| LoweringError::AllocationFailed)?;
    Ok(result)
}
pub(super) fn collect_exact<T>(
    items: impl ExactSizeIterator<Item = T>,
) -> Result<Vec<T>, LoweringError> {
    let mut result = reserved(items.len())?;
    result.extend(items);
    Ok(result)
}
pub(super) fn sum_lengths(mut items: impl Iterator<Item = usize>) -> Result<usize, LoweringError> {
    items.try_fold(0usize, |total, n| {
        total.checked_add(n).ok_or(LoweringError::CapacityOverflow)
    })
}
pub(super) struct PlacementNodes<'a>(pub(super) &'a [conduit_core::PlannedGear]);
impl PlacementNodes<'_> {
    pub(super) fn get(&self, id: &PlacementId) -> Option<NodeId> {
        self.0
            .iter()
            .position(|p| &p.placement_id == id)
            .and_then(|i| u16::try_from(i).ok())
            .map(NodeId)
    }
}
pub(super) struct NodeTables {
    pub(super) nodes: Vec<LoweredNode>,
    pub(super) specs: Vec<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>,
    pub(super) ports: Vec<KernelPortIdentity>,
}
pub(super) fn fore_connection_id(
    planned: &conduit_core::PlannedForePort,
) -> Result<ConnectionId, LoweringError> {
    let length = 10usize
        .checked_add(planned.front_port_id.as_str().len())
        .and_then(|n| n.checked_add(planned.placement_id.as_str().len()))
        .and_then(|n| n.checked_add(planned.gear_port_id.as_str().len()))
        .ok_or(LoweringError::CapacityOverflow)?;
    let mut id = alloc::string::String::new();
    id.try_reserve_exact(length)
        .map_err(|_| LoweringError::AllocationFailed)?;
    use core::fmt::Write;
    write!(
        &mut id,
        "front/{}/{}/{}/{}",
        planned.direction as u8,
        planned.front_port_id.as_str(),
        planned.placement_id.as_str(),
        planned.gear_port_id.as_str()
    )
    .map_err(|_| LoweringError::CapacityOverflow)?;
    Ok(ConnectionId::from(id))
}
pub(super) fn nodes(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<NodeTables, LoweringError> {
    let mut nodes = reserved(fragment.placements.len())?;
    let mut node_specs = reserved(fragment.placements.len())?;
    let mut identity_ports = reserved(sum_lengths(
        fragment
            .placements
            .iter()
            .flat_map(|p| [p.inputs.len(), p.outputs.len()]),
    )?)?;
    for (node_index, placement) in fragment.placements.iter().enumerate() {
        let node = NodeId(as_u16(node_index)?);
        if fragment.placements[..node_index]
            .iter()
            .any(|prior| prior.placement_id == placement.placement_id)
        {
            return Err(LoweringError::DuplicatePlacement(
                placement.placement_id.clone(),
            ));
        }
        let mut inputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.inputs,
            PortDirection::Input,
            placement.semantic_contract.value_contracts(),
        )?;
        let mut outputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.outputs,
            PortDirection::Output,
            placement.semantic_contract.value_contracts(),
        )?;
        for port in inputs.iter_mut().chain(outputs.iter_mut()) {
            port.maximum_value_bytes =
                placement.port_value_maximum_bytes(port.direction, &port.port_id, false);
            port.maximum_abnormal_value_bytes =
                placement.port_value_maximum_bytes(port.direction, &port.port_id, true);
        }
        if inputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Input,
                required: inputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        if outputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Output,
                required: outputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        let input_cords = [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE];
        let maximum_step_fuel = step_work::maximum_step_fuel(placement)?;
        let terminal_transductions = placement
            .terminal_transductions
            .iter()
            .map(|profile| {
                let input = inputs
                    .iter()
                    .find(|port| port.port_id == profile.input_port_id)
                    .map(|port| port.port)
                    .ok_or_else(|| {
                        LoweringError::InvalidTerminalTransduction(placement.placement_id.clone())
                    })?;
                let output = outputs
                    .iter()
                    .find(|port| port.port_id == profile.output_port_id)
                    .map(|port| port.port)
                    .ok_or_else(|| {
                        LoweringError::InvalidTerminalTransduction(placement.placement_id.clone())
                    })?;
                let cancellation_input = if matches!(
                    profile.cancellation,
                    conduit_core::CancellationTransduction::Request { .. }
                ) {
                    Some(
                        inputs
                            .iter()
                            .find(|port| {
                                port.value_kind.as_str()
                                    == conduit_core::CANCELLATION_REQUEST_INFO_ID
                            })
                            .map(|port| port.port)
                            .ok_or_else(|| {
                                LoweringError::InvalidTerminalTransduction(
                                    placement.placement_id.clone(),
                                )
                            })?,
                    )
                } else {
                    None
                };
                Ok(LoweredTerminalTransduction {
                    input,
                    output,
                    cancellation_input,
                    profile: profile.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        identity_ports.extend(
            inputs
                .iter()
                .chain(outputs.iter())
                .map(|port| KernelPortIdentity {
                    node,
                    direction: port.direction,
                    port: port.port,
                    port_id: port.port_id.clone(),
                }),
        );
        nodes.push(LoweredNode {
            node,
            placement_id: placement.placement_id.clone(),
            maximum_step_fuel,
            inputs,
            outputs,
            terminal_transductions,
        });
        node_specs.push(NodeSpec {
            input_cords,
            maximum_step_fuel,
        });
    }

    Ok(NodeTables {
        nodes,
        specs: node_specs,
        ports: identity_ports,
    })
}
