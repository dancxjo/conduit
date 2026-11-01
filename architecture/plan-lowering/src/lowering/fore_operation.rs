//! Bounded borrowed audit for a local Fore operation. No whole lowering or
//! identity map is retained. This is structural evidence, never native custody.
use super::*;
/// Exact projection from immutable Core-verified metadata. Private fields keep
/// caller-supplied numeric node/call records out of the binding entrance.
pub struct VerifiedForeOperationProjection<'a> {
    pub(crate) plan: &'a conduit_core::VerifiedPlanMetadata,
    pub(crate) fragment: &'a PlanFragment,
    pub(crate) node: LoweredNode,
    pub(crate) call: LoweredHostCall,
}
impl<'a> VerifiedForeOperationProjection<'a> {
    pub fn prepare(
        plan: &'a conduit_core::VerifiedPlanMetadata,
        fragment_id: &FragmentId,
        source: &conduit_core::PlotIdentity,
        fore: &[conduit_core::PlannedForePort],
        placement: &PlacementId,
    ) -> Result<Self, LoweringError> {
        let fragment = plan
            .preparation_metadata()
            .fragments
            .iter()
            .find(|fragment| &fragment.fragment_id == fragment_id)
            .ok_or(LoweringError::InvalidFragment)?;
        if fragment.source_document_id != source.source_document_id
            || fragment.checked_plot_id != source.checked_plot_id
            || fragment.expanded_plot_id != source.expanded_plot_id
            || fragment.fore_ports != fore
            || fore.is_empty()
        {
            return Err(LoweringError::InvalidFragment);
        }
        audit(fragment)?;
        let index = fragment
            .placements
            .iter()
            .position(|g| &g.placement_id == placement)
            .ok_or(LoweringError::InvalidFragment)?;
        let gear = &fragment.placements[index];
        let node = NodeId(as_u16(index)?);
        let contracts = gear.semantic_contract.value_contracts();
        let inputs = lower_ports(
            node,
            placement,
            &gear.inputs,
            PortDirection::Input,
            contracts,
        )?;
        let outputs = lower_ports(
            node,
            placement,
            &gear.outputs,
            PortDirection::Output,
            contracts,
        )?;
        let [required] = gear.host_calls.as_slice() else {
            return Err(LoweringError::InvalidFragment);
        };
        let maximum_step_fuel = as_u16(1 + inputs.len() + outputs.len() + gear.host_calls.len())?;
        Ok(Self {
            plan,
            fragment,
            node: LoweredNode {
                node,
                placement_id: placement.clone(),
                maximum_step_fuel,
                inputs,
                outputs,
                terminal_transductions: Vec::new(),
            },
            call: LoweredHostCall {
                node,
                call: HostCallId(0),
                contract_id: required.contract_id.clone(),
                target_kind: required.target_kind.clone(),
                maximum_in_flight: required.maximum_in_flight,
                binding: HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: required.maximum_input_bytes,
                    maximum_output_bytes: required.maximum_output_bytes,
                },
            },
        })
    }
    /// Owned numeric output is not a verified projection witness and cannot
    /// enter the selected binding entrance again.
    pub fn into_numeric(self) -> (LoweredNode, LoweredHostCall) {
        (self.node, self.call)
    }
    pub fn node(&self) -> &LoweredNode {
        &self.node
    }
    pub fn call(&self) -> &LoweredHostCall {
        &self.call
    }
    pub fn source(&self) -> conduit_core::PlotIdentity {
        conduit_core::PlotIdentity {
            source_document_id: self.fragment.source_document_id.clone(),
            checked_plot_id: self.fragment.checked_plot_id.clone(),
            expanded_plot_id: self.fragment.expanded_plot_id.clone(),
        }
    }
    pub fn fore(&self) -> &[conduit_core::PlannedForePort] {
        &self.fragment.fore_ports
    }
}
fn invalid<T>() -> Result<T, LoweringError> {
    Err(LoweringError::InvalidFragment)
}
fn port<'a>(
    gear: &'a conduit_core::PlannedGear,
    direction: PortDirection,
    id: &PlanPortId,
) -> Result<(usize, &'a conduit_core::PortDescriptor), LoweringError> {
    let ports = match direction {
        PortDirection::Input => &gear.inputs,
        PortDirection::Output => &gear.outputs,
    };
    ports
        .iter()
        .enumerate()
        .find(|(_, p)| &p.port_id == id)
        .ok_or(LoweringError::InvalidFragment)
}
fn bound(
    gear: &conduit_core::PlannedGear,
    direction: PortDirection,
    id: &PlanPortId,
    abnormal: bool,
) -> Option<u64> {
    use conduit_core::FrontValueLocation as L;
    gear.semantic_contract
        .value_contracts()
        .iter()
        .find(|b| match (&b.location, direction, abnormal) {
            (L::Input(p), PortDirection::Input, false)
            | (L::Output(p), PortDirection::Output, false)
            | (L::InputAbnormal(p), PortDirection::Input, true)
            | (L::OutputAbnormal(p), PortDirection::Output, true) => p == id,
            _ => false,
        })
        .map(|b| u64::from(b.contract.maximum_bytes))
}
/// All rich-lowering constraints for this deliberately bounded local subset.
/// Wider/stateful/remote graphs retain the existing full lowering entrance.
fn audit(fragment: &PlanFragment) -> Result<(), LoweringError> {
    admission::validate_verified_fragment(fragment, FIXED_KERNEL_STORAGE_PROFILE)?;
    if fragment.placements.is_empty()
        || fragment.placements.len() > 3
        || fragment.connections.len() > 3
        || fragment.fore_ports.len() > 3
        || !fragment.states.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_regions.is_empty()
    {
        return invalid();
    }
    let mut input_used = [[false; 2]; 3];
    for (i, gear) in fragment.placements.iter().enumerate() {
        if fragment.placements[..i]
            .iter()
            .any(|g| g.placement_id == gear.placement_id)
            || !gear.terminal_transductions.is_empty()
            || gear.host_calls.len() > 1
            || gear.resources.len() > usize::from(u16::MAX)
            || gear.host_calls.iter().any(|h| h.maximum_in_flight != 1)
            || gear.resources.iter().any(|r| r.units == 0)
        {
            return invalid();
        }
        for (direction, ports) in [
            (PortDirection::Input, &gear.inputs),
            (PortDirection::Output, &gear.outputs),
        ] {
            if ports.len() > 2 {
                return invalid();
            }
            for (p, descriptor) in ports.iter().enumerate() {
                if descriptor.direction != direction
                    || ports[..p]
                        .iter()
                        .any(|prior| prior.port_id == descriptor.port_id)
                {
                    return invalid();
                }
            }
        }
    }
    let node_for = |id: &PlacementId| {
        fragment
            .placements
            .iter()
            .position(|g| &g.placement_id == id)
            .ok_or(LoweringError::InvalidFragment)
    };
    let mut items = 0u16;
    let mut bytes = 0u32;
    for (index, cord) in fragment.connections.iter().enumerate() {
        if fragment.connections[..index]
            .iter()
            .any(|c| c.connection_id == cord.connection_id)
            || cord.resource.is_some()
            || cord.selected_line.is_some()
            || !cord.admitted_lines.is_empty()
            || cord.item_capacity == 0
            || cord.byte_capacity == 0
        {
            return invalid();
        }
        let source = &fragment.placements[node_for(&cord.source_placement_id)?];
        let sink_node = node_for(&cord.sink_placement_id)?;
        let sink = &fragment.placements[sink_node];
        let (_, source_port) = port(source, PortDirection::Output, &cord.source_port_id)?;
        let (sink_index, sink_port) = port(sink, PortDirection::Input, &cord.sink_port_id)?;
        if !source_contract_fields_match(
            &source_port.value_kind,
            source_port.abnormal_kind.as_ref(),
            source_port.temporal,
            cord.track,
            &cord.value_kind,
            cord.temporal,
        ) || sink_port.value_kind != cord.value_kind
            || (sink_port.temporal != cord.temporal
                && !matches!(
                    (cord.temporal, sink_port.temporal),
                    (
                        conduit_core::PortTemporal::Flow { .. },
                        conduit_core::PortTemporal::Value
                    ) | (
                        conduit_core::PortTemporal::Flow { closes: true },
                        conduit_core::PortTemporal::Flow { closes: false }
                    )
                ))
            || core::mem::replace(&mut input_used[sink_node][sink_index], true)
        {
            return invalid();
        }
        if !matches!(
            cord.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) {
            let a = bound(
                source,
                PortDirection::Output,
                &cord.source_port_id,
                cord.track == ConnectionTrack::AbnormalTerminal,
            );
            let b = bound(sink, PortDirection::Input, &cord.sink_port_id, false);
            if a.zip(b).is_some_and(|(a, b)| a != b) {
                return invalid();
            }
            if a.or(b).is_some_and(|n| u32::try_from(n).is_err()) {
                return invalid();
            }
        }
        items = items
            .checked_add(cord.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        bytes = bytes
            .checked_add(cord.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    for fore in &fragment.fore_ports {
        if fore.item_capacity == 0 || fore.byte_capacity == 0 || fore.selected_line.is_some() {
            return invalid();
        }
        let node = node_for(&fore.placement_id)?;
        let (index, descriptor) = port(
            &fragment.placements[node],
            fore.direction,
            &fore.gear_port_id,
        )?;
        if !fore_value::fields_match(
            &descriptor.value_kind,
            descriptor.abnormal_kind.as_ref(),
            descriptor.temporal,
            fore,
        ) {
            return invalid();
        }
        fore_value::selected_profile(fragment, fore)?;
        if fore.direction == PortDirection::Input
            && core::mem::replace(&mut input_used[node][index], true)
        {
            return invalid();
        }
        items = items
            .checked_add(fore.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        bytes = bytes
            .checked_add(fore.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    as_u16(fragment.expected_sign.len())?;
    for sign in &fragment.expected_sign {
        match sign {
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                node_for(id)?;
            }
            ExpectedSign::ConnectionTerminal(id) => {
                if !fragment.connections.iter().any(|c| &c.connection_id == id)
                    && !fragment.fore_ports.iter().any(|fore| {
                        id.as_str()
                            == alloc::format!(
                                "front/{}/{}/{}/{}",
                                fore.direction as u8,
                                fore.front_port_id.as_str(),
                                fore.placement_id.as_str(),
                                fore.gear_port_id.as_str()
                            )
                    })
                {
                    return invalid();
                }
            }
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {}
        }
    }
    // At most six Cords/routes/targets and three nodes fit canonical u16
    // numbering. No State/Fusion/pool/resource-Cord projection is omitted.
    let _ = (items, bytes);
    Ok(())
}
