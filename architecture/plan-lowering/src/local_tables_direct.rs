//! Fixed numeric local projection borrowing complete immutable verified facts.
use super::LocalTableRefusal as E;
use crate::lowering::{
    admission, admitted_maximum_value_bytes, lower_connection_track, lower_pressure_policy,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE, FIXED_KERNEL_STORAGE_PROFILE,
};
use conduit_core as c;
use conduit_kernel::{
    self as k,
    scheduler::{CordCapacity, CordSpec, NodeSpec},
};

pub struct DirectLocalKernelTables<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    plan: &'a c::VerifiedPlanMetadata,
    fragment: &'a c::PlanFragment,
    nodes: [NodeSpec<P>; N],
    cords: [CordSpec; C],
    routes: k::FixedRoutes<R, T>,
    calls: k::FixedHostCallBindings<H>,
    startup_order: [k::NodeId; N],
    cord_value_slots: u16,
    cord_value_bytes: u32,
    sign_items: u16,
    sign_bytes: u32,
}
/// Owned numeric storage after preparation/binding. This carries no checked
/// projection witness and cannot be passed back as verified preparation scope.
pub struct DirectLocalKernelParts<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    pub nodes: [NodeSpec<P>; N],
    pub cords: [CordSpec; C],
    pub routes: k::FixedRoutes<R, T>,
    pub calls: k::FixedHostCallBindings<H>,
    pub startup_order: [k::NodeId; N],
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

impl<
        'a,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > DirectLocalKernelTables<'a, N, C, P, R, T, H>
{
    // Retire numeric construction temporaries before resident/native checks.
    #[inline(never)]
    pub fn prepare(plan: &'a c::VerifiedPlanMetadata, id: &c::FragmentId) -> Result<Self, E> {
        // Borrow directly from the opaque verified owner; no cloned identity map.
        let fragment = plan
            .preparation_metadata()
            .fragments
            .iter()
            .find(|fragment| &fragment.fragment_id == id)
            .ok_or(E::Scope)?;
        admission::validate_verified_fragment(fragment, FIXED_KERNEL_STORAGE_PROFILE)
            .map_err(|_| E::Lowering)?;
        let parts = prepare_parts(fragment.into())?;
        Ok(Self {
            plan,
            fragment,
            nodes: parts.nodes,
            cords: parts.cords,
            routes: parts.routes,
            calls: parts.calls,
            startup_order: parts.startup_order,
            cord_value_slots: parts.cord_value_slots,
            cord_value_bytes: parts.cord_value_bytes,
            sign_items: parts.sign_items,
            sign_bytes: parts.sign_bytes,
        })
    }

    pub fn nodes(&self) -> &[NodeSpec<P>; N] {
        &self.nodes
    }
    pub fn cords(&self) -> &[CordSpec; C] {
        &self.cords
    }
    pub fn routes(&self) -> &k::FixedRoutes<R, T> {
        &self.routes
    }
    pub fn calls(&self) -> &k::FixedHostCallBindings<H> {
        &self.calls
    }
    pub fn cord_value_slots(&self) -> u16 {
        self.cord_value_slots
    }
    pub fn cord_value_bytes(&self) -> u32 {
        self.cord_value_bytes
    }
    pub fn sign_items(&self) -> u16 {
        self.sign_items
    }
    pub fn sign_bytes(&self) -> u32 {
        self.sign_bytes
    }
    pub fn into_parts(self) -> DirectLocalKernelParts<N, C, P, R, T, H> {
        DirectLocalKernelParts {
            nodes: self.nodes,
            cords: self.cords,
            routes: self.routes,
            calls: self.calls,
            startup_order: self.startup_order,
            cord_value_slots: self.cord_value_slots,
            cord_value_bytes: self.cord_value_bytes,
            sign_items: self.sign_items,
            sign_bytes: self.sign_bytes,
        }
    }
    pub fn preparation_scope(&self) -> (&'a c::VerifiedPlanMetadata, &'a c::PlanFragment) {
        (self.plan, self.fragment)
    }
    pub fn node_for_placement(&self, id: &c::PlacementId) -> Option<k::NodeId> {
        self.fragment
            .placements
            .iter()
            .position(|g| &g.placement_id == id)
            .map(|n| k::NodeId(n as u16))
    }
    pub fn startup_order(&self) -> &[k::NodeId; N] {
        &self.startup_order
    }
}
fn bound(
    gear: c::PlannedGearRef<'_>,
    id: &str,
    direction: c::PortDirection,
    abnormal: bool,
) -> Option<u32> {
    use c::FrontValueLocation as L;
    gear.semantic_contract
        .value_contracts()
        .iter()
        .find_map(|entry| {
            let matches = match (&entry.location, direction, abnormal) {
                (L::Input(port), c::PortDirection::Input, false)
                | (L::Output(port), c::PortDirection::Output, false)
                | (L::InputAbnormal(port), c::PortDirection::Input, true)
                | (L::OutputAbnormal(port), c::PortDirection::Output, true) => port.as_str() == id,
                _ => false,
            };
            matches.then_some(entry.contract.maximum_bytes)
        })
}

#[derive(Clone, Copy)]
pub(super) struct DirectFragmentFacts<'a> {
    pub header: c::FragmentFingerprintHeaderRef<'a>,
    pub placements: c::PlannedGearRefs<'a>,
    pub tail: c::FragmentFingerprintTailRef<'a>,
}
impl<'a> From<&'a c::PlanFragment> for DirectFragmentFacts<'a> {
    fn from(fragment: &'a c::PlanFragment) -> Self {
        Self {
            header: fragment.into(),
            placements: c::PlannedGearRefs::Owned(&fragment.placements),
            tail: fragment.into(),
        }
    }
}
impl<'a> From<c::FlatPlanRef<'a>> for DirectFragmentFacts<'a> {
    fn from(fragment: c::FlatPlanRef<'a>) -> Self {
        Self {
            header: fragment.header,
            placements: fragment.placements,
            tail: fragment.tail,
        }
    }
}
fn prepare_parts<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    fragment: DirectFragmentFacts<'_>,
) -> Result<DirectLocalKernelParts<N, C, P, R, T, H>, E> {
    if N == 0
        || P == 0
        || N > usize::from(u16::MAX)
        || P > usize::from(u16::MAX)
        || C > usize::from(u16::MAX)
        || fragment.placements.len() != N
    {
        return Err(E::Capacity);
    }
    if !fragment.tail.fore_ports.is_empty()
        || !fragment.tail.states.is_empty()
        || !fragment.header.execution_fusions.is_empty()
        || !fragment.tail.shared_pools.is_empty()
        || fragment
            .placements
            .iter()
            .any(|g| !g.terminal_transductions.is_empty())
        || fragment
            .tail
            .connections
            .iter()
            .any(|c| c.resource.is_some())
    {
        return Err(E::Unsupported);
    }
    if fragment.tail.connections.len() != C {
        return Err(E::Capacity);
    }
    if fragment.header.execution_regions.len() > 1
        || fragment.header.execution_regions.iter().any(|r| {
            r.lane_count != 1
                || r.scheduling != c::ExecutionScheduling::CooperativeBoundedStep
                || r.admitted_placements.len() != N
        })
    {
        return Err(E::Unsupported);
    }
    let mut nodes = [NodeSpec {
        input_cords: [None; P],
        maximum_step_fuel: 0,
    }; N];
    for (index, (spec, gear)) in nodes.iter_mut().zip(fragment.placements.iter()).enumerate() {
        if fragment
            .placements
            .iter()
            .take(index)
            .any(|g| g.placement_id == gear.placement_id)
        {
            return Err(E::Lowering);
        }
        for (direction, ports) in [
            (c::PortDirection::Input, gear.inputs),
            (c::PortDirection::Output, gear.outputs),
        ] {
            if ports.len() > FIXED_KERNEL_STORAGE_PORTS_PER_NODE {
                return Err(E::Lowering);
            }
            if ports.len() > P {
                return Err(E::Capacity);
            }
            for (i, port) in ports.iter().enumerate() {
                if port.direction != direction
                    || ports.iter().take(i).any(|p| p.port_id == port.port_id)
                {
                    return Err(E::Lowering);
                }
            }
        }
        spec.maximum_step_fuel = u16::try_from(
            1usize
                .checked_add(gear.inputs.len())
                .and_then(|n| n.checked_add(gear.outputs.len()))
                .and_then(|n| n.checked_add(gear.host_calls.len()))
                .ok_or(E::Lowering)?,
        )
        .map_err(|_| E::Lowering)?;
    }
    let node_for = |id: &str| {
        fragment
            .placements
            .iter()
            .position(|g| g.placement_id == id)
            .map(|n| k::NodeId(n as u16))
    };
    let mut cords = [CordSpec::inactive(); C];
    let mut value_slots = 0u16;
    let mut value_bytes = 0u32;
    for (index, (out, connection)) in cords
        .iter_mut()
        .zip(fragment.tail.connections.iter())
        .enumerate()
    {
        if fragment
            .tail
            .connections
            .iter()
            .take(index)
            .any(|c| c.connection_id == connection.connection_id)
            || connection.item_capacity == 0
            || connection.byte_capacity == 0
        {
            return Err(E::Lowering);
        }
        let source_node = node_for(connection.source_placement_id).ok_or(E::Unsupported)?;
        let sink_node = node_for(connection.sink_placement_id).ok_or(E::Unsupported)?;
        let source = fragment
            .placements
            .iter()
            .nth(usize::from(source_node.0))
            .ok_or(E::Lowering)?;
        let sink = fragment
            .placements
            .iter()
            .nth(usize::from(sink_node.0))
            .ok_or(E::Lowering)?;
        let source_port = source
            .outputs
            .iter()
            .position(|p| p.port_id == connection.source_port_id)
            .ok_or(E::Lowering)?;
        let sink_port = sink
            .inputs
            .iter()
            .position(|p| p.port_id == connection.sink_port_id)
            .ok_or(E::Lowering)?;
        let source_descriptor = source.outputs.get(source_port).ok_or(E::Lowering)?;
        let sink_descriptor = sink.inputs.get(sink_port).ok_or(E::Lowering)?;
        let source_matches = crate::lowering::source_contract_fields_match_ref(
            source_descriptor.value_kind,
            source_descriptor.abnormal_kind,
            source_descriptor.temporal,
            connection.track,
            connection.value_kind,
            connection.temporal,
        );
        let sink_matches = sink_descriptor.value_kind == connection.value_kind
            && (sink_descriptor.temporal == connection.temporal
                || matches!(
                    (connection.temporal, sink_descriptor.temporal),
                    (c::PortTemporal::Flow { .. }, c::PortTemporal::Value)
                        | (
                            c::PortTemporal::Flow { closes: true },
                            c::PortTemporal::Flow { closes: false }
                        )
                ));
        if !source_matches || !sink_matches {
            return Err(E::Lowering);
        }
        let terminal = matches!(
            connection.track,
            c::ConnectionTrack::NormalClose | c::ConnectionTrack::Quiescence
        );
        let source_bound = match connection.track {
            c::ConnectionTrack::Payload => bound(
                source,
                connection.source_port_id,
                c::PortDirection::Output,
                false,
            ),
            c::ConnectionTrack::AbnormalTerminal => bound(
                source,
                connection.source_port_id,
                c::PortDirection::Output,
                true,
            ),
            _ => None,
        };
        let sink_bound = bound(
            sink,
            connection.sink_port_id,
            c::PortDirection::Input,
            false,
        );
        if !terminal && source_bound.zip(sink_bound).is_some_and(|(a, b)| a != b) {
            return Err(E::Lowering);
        }
        let maximum = if terminal {
            connection.byte_capacity
        } else {
            admitted_maximum_value_bytes(source_bound.or(sink_bound), connection.byte_capacity)
        };
        if connection.selected_line.is_some() || !connection.admitted_lines.is_empty() {
            return Err(E::Lowering);
        }
        let cord = k::CordId(index as u16);
        if nodes[usize::from(sink_node.0)].input_cords[sink_port]
            .replace(cord)
            .is_some()
        {
            return Err(E::Lowering);
        }
        *out = CordSpec::local(
            cord,
            (source_node, k::PortId(source_port as u16)),
            (sink_node, k::PortId(sink_port as u16)),
            CordCapacity {
                slot_start: value_slots,
                item_capacity: connection.item_capacity,
                byte_capacity: connection.byte_capacity,
                pressure_policy: lower_pressure_policy(connection.pressure_policy),
            },
        )
        .with_track(lower_connection_track(connection.track))
        .with_maximum_value_bytes(maximum);
        value_slots = value_slots
            .checked_add(connection.item_capacity)
            .ok_or(E::Lowering)?;
        value_bytes = value_bytes
            .checked_add(connection.byte_capacity)
            .ok_or(E::Lowering)?;
    }
    let mut routes = k::FixedRoutes::new(P as u16);
    let mut targets = [k::RouteTarget {
        cord: k::CordId(0),
        sink: k::CordEndpoint::Local {
            node: k::NodeId(0),
            port: k::PortId(0),
        },
    }; T];
    let mut next_target = 0u16;
    let mut route_count = 0usize;
    for node in 0..N {
        for port in 0..P {
            let mut len = 0usize;
            for cord in &cords {
                if cord.source_local() == Some((k::NodeId(node as u16), k::PortId(port as u16))) {
                    if len >= T {
                        return Err(E::Kernel(k::ProtocolError::RouteTargetExceeded));
                    }
                    targets[len] = k::RouteTarget {
                        cord: cord.cord,
                        sink: cord.sink,
                    };
                    len += 1;
                }
            }
            if len != 0 {
                route_count += 1;
                if route_count > R {
                    return Err(E::Capacity);
                }
                let len16 = u16::try_from(len).map_err(|_| E::Lowering)?;
                routes
                    .install(
                        k::NodeId(node as u16),
                        k::PortId(port as u16),
                        k::RouteRange {
                            start: next_target,
                            len: len16,
                        },
                        &targets[..len],
                    )
                    .map_err(E::Kernel)?;
                next_target = next_target.checked_add(len16).ok_or(E::Lowering)?;
            }
        }
    }
    routes.seal().map_err(E::Kernel)?;
    let mut calls = k::FixedHostCallBindings::new(1);
    let mut call_count = 0usize;
    for (node, gear) in fragment.placements.iter().enumerate() {
        for (call, requirement) in gear.host_calls.iter().enumerate() {
            if requirement.maximum_in_flight != 1 {
                return Err(E::Lowering);
            }
            if call != 0 {
                return Err(E::Unsupported);
            }
            call_count += 1;
            if call_count > H {
                return Err(E::Capacity);
            }
            calls
                .install(
                    k::NodeId(node as u16),
                    k::HostCallBinding {
                        call: k::HostCallId(0),
                        maximum_input_bytes: requirement.maximum_input_bytes,
                        maximum_output_bytes: requirement.maximum_output_bytes,
                    },
                )
                .map_err(E::Kernel)?;
        }
        for (index, resource) in gear.resources.iter().enumerate() {
            if resource.units == 0 || index > usize::from(u16::MAX) {
                return Err(E::Lowering);
            }
            if gear.host_calls.is_empty() {
                return Err(E::Unsupported);
            }
        }
    }
    calls.seal().map_err(E::Kernel)?;
    for (index, expected) in fragment.tail.expected_sign.iter().enumerate() {
        if index > usize::from(u16::MAX) {
            return Err(E::Lowering);
        }
        match expected {
            c::ExpectedSignRef::PlacementPrepared(id)
            | c::ExpectedSignRef::PlacementTerminal(id)
                if node_for(id).is_none() =>
            {
                return Err(E::Lowering)
            }
            c::ExpectedSignRef::ConnectionTerminal(id)
                if !fragment
                    .tail
                    .connections
                    .iter()
                    .any(|c| c.connection_id == id) =>
            {
                return Err(E::Lowering)
            }
            _ => (),
        }
    }
    if fragment.tail.startup_order.len() != N {
        return Err(E::Scope);
    }
    let mut startup_order = [k::NodeId(u16::MAX); N];
    for (out, id) in startup_order
        .iter_mut()
        .zip(fragment.tail.startup_order.iter())
    {
        *out = node_for(id).ok_or(E::Scope)?;
    }
    Ok(DirectLocalKernelParts {
        nodes,
        cords,
        routes,
        calls,
        startup_order,
        cord_value_slots: value_slots,
        cord_value_bytes: value_bytes,
        sign_items: fragment.tail.sign_storage_budget.item_capacity,
        sign_bytes: fragment.tail.sign_storage_budget.byte_capacity,
    })
}

/// Numeric tables that retain the actual opaque Core flat verification scope.
/// This is neither Source/current-offer admission nor native owner possession.
pub struct FlatDirectLocalKernelTables<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    plan: &'a c::VerifiedFlatPlanRef<'a>,
    parts: DirectLocalKernelParts<N, C, P, R, T, H>,
}
impl<
        'a,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > FlatDirectLocalKernelTables<'a, N, C, P, R, T, H>
{
    pub fn prepare(plan: &'a c::VerifiedFlatPlanRef<'a>) -> Result<Self, E> {
        let fragment = DirectFragmentFacts::from(plan.view());
        admission::validate_verified_fragment_facts(
            fragment.header,
            fragment.placements,
            fragment.tail,
            FIXED_KERNEL_STORAGE_PROFILE,
        )
        .map_err(|_| E::Lowering)?;
        Ok(Self {
            plan,
            parts: prepare_parts(fragment)?,
        })
    }
    pub fn preparation_scope(&self) -> &'a c::VerifiedFlatPlanRef<'a> {
        self.plan
    }
    pub fn nodes(&self) -> &[NodeSpec<P>; N] {
        &self.parts.nodes
    }
    pub fn cords(&self) -> &[CordSpec; C] {
        &self.parts.cords
    }
    pub fn routes(&self) -> &k::FixedRoutes<R, T> {
        &self.parts.routes
    }
    pub fn calls(&self) -> &k::FixedHostCallBindings<H> {
        &self.parts.calls
    }
    pub fn cord_value_slots(&self) -> u16 {
        self.parts.cord_value_slots
    }
    pub fn cord_value_bytes(&self) -> u32 {
        self.parts.cord_value_bytes
    }
    pub fn sign_items(&self) -> u16 {
        self.parts.sign_items
    }
    pub fn sign_bytes(&self) -> u32 {
        self.parts.sign_bytes
    }
    pub fn startup_order(&self) -> &[k::NodeId; N] {
        &self.parts.startup_order
    }
    pub fn node_for_placement(&self, id: &str) -> Option<k::NodeId> {
        self.plan
            .view()
            .placements
            .iter()
            .position(|g| g.placement_id == id)
            .map(|n| k::NodeId(n as u16))
    }
    pub fn into_parts(self) -> DirectLocalKernelParts<N, C, P, R, T, H> {
        self.parts
    }
}
