pub mod analysis;
pub mod plan;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use conduit_core::{
    state_resource_budget, GearId, PlannedStateBoundary, StatePlanError, StateResourceBudget,
};
use conduit_form::{CheckedConnection, CheckedForm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedStateGraph {
    pub form_identity: conduit_core::FormIdentity,
    pub startup_order: Vec<GearId>,
    pub states: Vec<PlannedStateBoundary>,
    pub resources: StateResourceBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateGraphError {
    InvalidForm,
    InvalidPlan,
    StateAlreadySealed,
    InvalidStatePlan(StatePlanError),
    UnknownStatePlacement,
    MissingInitialization,
    KindMismatch,
    MissingNextWriter,
    MultipleNextWriters,
    OrdinaryCycle,
}

/// Validates one logical step. Connections entering a declared state are the
/// candidate-next edge and are cut for cycle checking; all other edges remain
/// ordinary and must form a DAG.
pub fn admit_state_graph(
    form: &CheckedForm,
    states: Vec<PlannedStateBoundary>,
) -> Result<AdmittedStateGraph, StateGraphError> {
    form.validate_identities()
        .map_err(|_| StateGraphError::InvalidForm)?;
    let resources = state_resource_budget(&states).map_err(StateGraphError::InvalidStatePlan)?;
    for state in &states {
        let gear = form
            .gears
            .iter()
            .find(|gear| gear.gear_id == state.gear_id)
            .ok_or(StateGraphError::UnknownStatePlacement)?;
        let current_matches = gear
            .outputs
            .iter()
            .any(|port| port.value_kind == state.value_kind);
        let next_matches = gear
            .inputs
            .iter()
            .any(|port| port.value_kind == state.value_kind);
        if !current_matches || !next_matches {
            return Err(StateGraphError::KindMismatch);
        }
        let writers = form
            .connections
            .iter()
            .filter(|connection| connection.sink_gear_id == state.gear_id)
            .count();
        match writers {
            0 => return Err(StateGraphError::MissingNextWriter),
            1 => {}
            _ => return Err(StateGraphError::MultipleNextWriters),
        }
    }
    let state_gears = states
        .iter()
        .map(|state| state.gear_id.clone())
        .collect::<BTreeSet<_>>();
    let order = acyclic_order(
        &form
            .gears
            .iter()
            .map(|gear| gear.gear_id.clone())
            .collect::<Vec<_>>(),
        &form.connections,
        &state_gears,
    )
    .ok_or(StateGraphError::OrdinaryCycle)?;
    Ok(AdmittedStateGraph {
        form_identity: form.identity(),
        startup_order: order,
        states,
        resources,
    })
}

fn acyclic_order(
    nodes: &[GearId],
    connections: &[CheckedConnection],
    state_gears: &BTreeSet<GearId>,
) -> Option<Vec<GearId>> {
    let mut indegree = nodes
        .iter()
        .cloned()
        .map(|node| (node, 0usize))
        .collect::<BTreeMap<_, _>>();
    for edge in connections {
        if !state_gears.contains(&edge.sink_gear_id) {
            *indegree.get_mut(&edge.sink_gear_id)? += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node) = ready.iter().next().cloned() {
        ready.remove(&node);
        order.push(node.clone());
        for edge in connections
            .iter()
            .filter(|edge| edge.source_gear_id == node && !state_gears.contains(&edge.sink_gear_id))
        {
            let count = indegree.get_mut(&edge.sink_gear_id)?;
            *count -= 1;
            if *count == 0 {
                ready.insert(edge.sink_gear_id.clone());
            }
        }
    }
    (order.len() == nodes.len()).then_some(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{KindId, PortId, PortTemporal};

    fn edge(source: &str, sink: &str) -> CheckedConnection {
        CheckedConnection {
            source_gear_id: GearId::from(source),
            source_port_id: PortId::from("current"),
            sink_gear_id: GearId::from(sink),
            sink_port_id: PortId::from("next"),
            value_kind: KindId::from("number/u32@1"),
            temporal: PortTemporal::Value,
        }
    }

    #[test]
    fn explicit_state_is_the_only_cycle_cut() {
        let nodes = vec![GearId::from("state"), GearId::from("step")];
        let edges = vec![edge("state", "step"), edge("step", "state")];
        assert!(acyclic_order(&nodes, &edges, &BTreeSet::new()).is_none());
        assert!(acyclic_order(&nodes, &edges, &BTreeSet::from([GearId::from("state")])).is_some());
    }

    #[test]
    fn ordinary_self_cycle_is_not_a_state_boundary() {
        let nodes = vec![GearId::from("step")];
        let edges = vec![edge("step", "step")];
        assert!(acyclic_order(&nodes, &edges, &BTreeSet::new()).is_none());
    }

    #[test]
    fn admitted_state_self_feedback_crosses_the_delay_boundary() {
        let nodes = vec![GearId::from("state")];
        let edges = vec![edge("state", "state")];
        assert_eq!(
            acyclic_order(&nodes, &edges, &BTreeSet::from([GearId::from("state")])),
            Some(nodes)
        );
    }

    #[test]
    fn unrelated_state_does_not_hide_an_ordinary_self_cycle() {
        let nodes = vec![GearId::from("state"), GearId::from("step")];
        let edges = vec![edge("state", "step"), edge("step", "step")];
        assert!(acyclic_order(&nodes, &edges, &BTreeSet::from([GearId::from("state")])).is_none());
    }

    #[test]
    fn ordinary_cycle_survives_an_unrelated_state_cut() {
        let nodes = vec![GearId::from("state"), GearId::from("a"), GearId::from("b")];
        let edges = vec![edge("a", "b"), edge("b", "a")];
        assert!(acyclic_order(&nodes, &edges, &BTreeSet::from([GearId::from("state")])).is_none());
    }
}
