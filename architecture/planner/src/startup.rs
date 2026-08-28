//! Deterministic placement startup ordering.
//!
//! Ordinary data dependencies remain acyclic. A placement may break a runtime
//! request/response loop only when its exact selected realization contains an
//! admitted zero-input host operation that produces one of its output kinds.
//! That is concrete Plan truth that the operation can begin from host input;
//! it is not an inference from a semantic kind name.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use conduit_core::{PlacementId, PlannedConnection, PlannedGear};

pub(crate) fn startup_order(
    placements: &[PlannedGear],
    connections: &[PlannedConnection],
) -> Option<Vec<PlacementId>> {
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
                            || &connection.source_placement_id != *candidate
                            || !remaining.contains(&connection.sink_placement_id)
                    })
            })
            .cloned()?;
        remaining.remove(&next);
        ordered.push(next);
    }
    Some(ordered)
}

fn starts_from_admitted_host_input(placement: &PlannedGear) -> bool {
    placement.host_operations.iter().any(|operation| {
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
