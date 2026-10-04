//! Deterministic placement startup ordering.
//!
//! Ordinary data dependencies remain acyclic. Exact Source-seeded state cuts
//! only its declared next input; initialization and ordinary cords remain dependencies.
//! A placement may also break a runtime
//! request/response loop only when its exact selected realization contains an
//! admitted zero-input Host Call that produces one of its output kinds.
//! That is concrete Plan truth that the operation can begin from host input;
//! it is not an inference from a semantic kind name.

use crate::PlannerError;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;
use conduit_core::{PlacementId, PlannedConnection, PlannedGear, PortId, StartupDependency};

pub(crate) fn startup_order(
    placements: &[PlannedGear],
    connections: &[PlannedConnection],
) -> Result<Option<Vec<PlacementId>>, PlannerError> {
    let delayed = delay_inputs(placements)?;
    let autonomous = placements
        .iter()
        .filter(|placement| starts_from_admitted_host_input(placement))
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut remaining = placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .find(|candidate| {
                autonomous.contains(*candidate)
                    || connections.iter().all(|connection| {
                        connection.source_placement_id == connection.sink_placement_id
                            || is_delayed(connection, &delayed)
                            || &connection.source_placement_id != *candidate
                            || !remaining.contains(&connection.sink_placement_id)
                    })
            })
            .cloned();
        let Some(next) = next else {
            return Ok(None);
        };
        remaining.remove(&next);
        ordered.push(next);
    }
    Ok(Some(ordered))
}

fn starts_from_admitted_host_input(placement: &PlannedGear) -> bool {
    placement.host_calls.iter().any(|operation| {
        operation.maximum_input_bytes == 0
            && operation.maximum_output_bytes > 0
            && operation.target_kind.as_ref().is_some_and(|target| {
                placement
                    .outputs
                    .iter()
                    .any(|output| output.value_kind == *target)
            })
    })
}

fn delay_inputs(placements: &[PlannedGear]) -> Result<BTreeMap<PlacementId, PortId>, PlannerError> {
    let mut delayed = BTreeMap::new();
    for placement in placements {
        let boundary = conduit_core::source_seeded_state_boundary(
            &placement.inputs,
            &placement.outputs,
            &placement.semantic_contract,
        )
        .map_err(|reason| {
            PlannerError::InvalidStateContract(format!(
                "{}: {reason}",
                placement.placement_id.as_str()
            ))
        })?;
        if let Some(boundary) = boundary {
            if placement.limits.max_active_instances == 0
                || placement.limits.max_queue_items < 2
                || boundary
                    .value
                    .maximum_bytes
                    .checked_mul(2)
                    .is_none_or(|bytes| placement.limits.max_queue_bytes < bytes)
            {
                return Err(PlannerError::InvalidStateContract(
                    placement.placement_id.as_str().into(),
                ));
            }
            delayed.insert(
                placement.placement_id.clone(),
                boundary.next_port_id.clone(),
            );
        }
    }
    Ok(delayed)
}

fn is_delayed(connection: &PlannedConnection, delayed: &BTreeMap<PlacementId, PortId>) -> bool {
    delayed.get(&connection.sink_placement_id) == Some(&connection.sink_port_id)
}

pub(crate) fn startup_dependencies(
    placements: &[PlannedGear],
    connections: &[PlannedConnection],
) -> Result<Vec<StartupDependency>, PlannerError> {
    let delayed = delay_inputs(placements)?;
    Ok(connections
        .iter()
        .filter(|connection| {
            connection.source_placement_id != connection.sink_placement_id
                && !is_delayed(connection, &delayed)
        })
        .map(|connection| StartupDependency {
            prerequisite_placement_id: connection.sink_placement_id.clone(),
            dependent_placement_id: connection.source_placement_id.clone(),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests;
