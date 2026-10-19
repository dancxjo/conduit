//! Recover a bounded physical DAG order from complete Plan-owned Cord facts.
use super::{lower_plan_fragment_from_plan, LoweredFusion, LoweringError};
use alloc::{collections::BTreeSet, vec, vec::Vec};
use conduit_core::{
    derive_planned_transformation_eligibility, ConnectionTrack, DeliveryPressurePolicy, FragmentId,
    Plan, PortTemporal, WorkTransformation,
};
use conduit_kernel::{CordEndpoint, NodeId};

/// Lower explicitly selected pure Value DAGs through the ordinary whole-Plan
/// entrance. This proves static graph eligibility, not Host implementation
/// support or a speedup. The Host must match the selected physical profile and
/// the kernel still preflights exact Back layouts and storage before Play.
pub fn lower_pure_value_fusion_dags(
    plan: &Plan,
    fragment_id: &FragmentId,
) -> Result<Vec<LoweredFusion>, LoweringError> {
    let (lowered, _) = lower_plan_fragment_from_plan(plan, fragment_id)?;
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| &fragment.fragment_id == fragment_id)
        .ok_or(LoweringError::InvalidFragment)?;
    let mut result = Vec::with_capacity(lowered.fusions.len());
    for fusion in &lowered.fusions {
        if !(2..=32).contains(&fusion.nodes.len()) {
            return Err(LoweringError::InvalidFragment);
        }
        let selected = fragment
            .execution_fusions
            .iter()
            .find(|selected| selected.fusion_id == fusion.fusion_id)
            .ok_or(LoweringError::InvalidFragment)?;
        let members: BTreeSet<_> = selected.preserved_placements.iter().collect();
        for node in &fusion.nodes {
            let identity = lowered
                .nodes
                .iter()
                .find(|identity| identity.node == *node)
                .ok_or(LoweringError::InvalidFragment)?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == identity.placement_id)
                .ok_or(LoweringError::InvalidFragment)?;
            derive_planned_transformation_eligibility(placement)
                .and_then(|eligibility| eligibility.require(WorkTransformation::Fusion))
                .map_err(|_| LoweringError::InvalidFragment)?;
            if placement
                .inputs
                .iter()
                .chain(&placement.outputs)
                .any(|port| port.temporal != PortTemporal::Value)
                || !identity.terminal_transductions.is_empty()
                || lowered.host_calls.iter().any(|call| call.node == *node)
                || lowered.states.iter().any(|state| state.node == *node)
            {
                return Err(LoweringError::InvalidFragment);
            }
        }
        if !fragment.execution_regions.is_empty()
            && !fragment.execution_regions.iter().any(|region| {
                !region.preemption_required
                    && !region.isolation_required
                    && selected
                        .preserved_placements
                        .iter()
                        .all(|id| region.admitted_placements.contains(id))
            })
        {
            return Err(LoweringError::InvalidFragment);
        }
        let internal: Vec<_> = fragment
            .connections
            .iter()
            .filter(|cord| {
                members.contains(&cord.source_placement_id)
                    && members.contains(&cord.sink_placement_id)
            })
            .collect();
        let exact: BTreeSet<_> = internal.iter().map(|cord| &cord.connection_id).collect();
        if exact.is_empty() || exact != selected.preserved_connections.iter().collect() {
            return Err(LoweringError::InvalidFragment);
        }
        for cord in &internal {
            if cord.track != ConnectionTrack::Payload
                || cord.temporal != PortTemporal::Value
                || cord.pressure_policy != DeliveryPressurePolicy::PreserveOrder
                || cord.resource.is_some()
                || cord.selected_line.is_some()
                || !cord.admitted_lines.is_empty()
                || cord.item_capacity == 0
                || cord.byte_capacity == 0
            {
                return Err(LoweringError::InvalidFragment);
            }
            // Partial internal/escaping fanout cannot be represented by one
            // compact output. The whole output remains a boundary instead.
            if fragment.connections.iter().any(|other| {
                other.source_placement_id == cord.source_placement_id
                    && other.source_port_id == cord.source_port_id
                    && !members.contains(&other.sink_placement_id)
            }) || fragment.fore_ports.iter().any(|fore| {
                fore.placement_id == cord.source_placement_id
                    && fore.gear_port_id == cord.source_port_id
            }) {
                return Err(LoweringError::InvalidFragment);
            }
        }
        let mut edges = Vec::with_capacity(fusion.cords.len());
        for id in &fusion.cords {
            let cord = lowered
                .cords
                .iter()
                .find(|cord| cord.spec.cord == *id)
                .ok_or(LoweringError::InvalidFragment)?;
            let (CordEndpoint::Local { node: source, .. }, CordEndpoint::Local { node: sink, .. }) =
                (cord.spec.source, cord.spec.sink)
            else {
                return Err(LoweringError::InvalidFragment);
            };
            edges.push((source, sink));
        }
        let mut connected = BTreeSet::from([fusion.nodes[0]]);
        for _ in 0..fusion.nodes.len() {
            for (a, b) in &edges {
                if connected.contains(a) || connected.contains(b) {
                    connected.insert(*a);
                    connected.insert(*b);
                }
            }
        }
        if connected.len() != fusion.nodes.len() {
            return Err(LoweringError::InvalidFragment);
        }
        let mut ordered: Vec<NodeId> = vec![];
        while ordered.len() < fusion.nodes.len() {
            let next = fusion
                .nodes
                .iter()
                .filter(|node| !ordered.contains(node))
                .filter(|node| {
                    edges
                        .iter()
                        .filter(|(_, sink)| sink == *node)
                        .all(|(source, _)| ordered.contains(source))
                })
                .min()
                .copied()
                .ok_or(LoweringError::InvalidFragment)?;
            ordered.push(next);
        }
        let mut bound = fusion.clone();
        bound.nodes = ordered;
        result.push(bound);
    }
    Ok(result)
}
