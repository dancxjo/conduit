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
                        candidate_dependency_allows_ref(
                            candidate.as_str(),
                            connection.into(),
                            is_delayed(connection, &delayed),
                            remaining.contains(&connection.sink_placement_id),
                        )
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
    starts_from_admitted_host_input_ref(placement.into())
}

fn delay_inputs(placements: &[PlannedGear]) -> Result<BTreeMap<PlacementId, PortId>, PlannerError> {
    let mut delayed = BTreeMap::new();
    for placement in placements {
        if let Some(law) = conduit_core::initialized_state_boundary(
            &placement.inputs,
            &placement.outputs,
            &placement.semantic_contract,
        )
        .map_err(|reason| {
            PlannerError::InvalidStateContract(format!(
                "{}: {reason}",
                placement.placement_id.as_str()
            ))
        })? {
            if placement.limits.max_active_instances == 0
                || placement.limits.max_queue_items == 0
                || placement.limits.max_queue_bytes < law.current.maximum_bytes
            {
                return Err(PlannerError::InvalidStateContract(
                    placement.placement_id.as_str().into(),
                ));
            }
            delayed.insert(placement.placement_id.clone(), law.update_port_id.clone());
        }
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
            dependency_required_ref((*connection).into(), is_delayed(connection, &delayed))
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

// Shared eligibility predicate: rich BTreeSet orchestration stays unchanged.
#[cfg(feature = "indexed-source")]
pub(crate) fn candidate_ready_ref(
    candidate: &str,
    autonomous: bool,
    connections: conduit_core::PlannedConnectionRefs<'_>,
    mut remaining: impl FnMut(&str) -> bool,
    mut delayed: impl FnMut(conduit_core::PlannedConnectionRef<'_>) -> bool,
) -> bool {
    autonomous
        || connections.iter().all(|connection| {
            candidate_dependency_allows_ref(
                candidate,
                connection,
                delayed(connection),
                remaining(connection.sink_placement_id),
            )
        })
}
pub(crate) fn starts_from_admitted_host_input_ref(p: conduit_core::PlannedGearRef<'_>) -> bool {
    p.host_calls.iter().any(|c| {
        c.maximum_input_bytes == 0
            && c.maximum_output_bytes > 0
            && c.target_kind
                .is_some_and(|target| p.outputs.iter().any(|port| port.value_kind == target))
    })
}
#[cfg(feature = "indexed-source")]
pub(crate) fn plain_startup_order_ref<'a>(
    placements: conduit_core::PlannedGearRefs<'a>,
    connections: &[conduit_core::PlannedConnectionRef<'a>],
) -> Result<Option<alloc::vec::Vec<&'a str>>, alloc::collections::TryReserveError> {
    let mut remaining = alloc::vec::Vec::new();
    remaining.try_reserve_exact(placements.len())?;
    remaining.extend(placements.iter().map(|p| p.placement_id));
    remaining.sort_unstable();
    remaining.dedup();
    let mut result = alloc::vec::Vec::new();
    result.try_reserve_exact(remaining.len())?;
    while !remaining.is_empty() {
        let next = remaining.iter().position(|id| {
            candidate_ready_ref(
                id,
                placements
                    .iter()
                    .any(|p| p.placement_id == *id && starts_from_admitted_host_input_ref(p)),
                conduit_core::PlannedConnectionRefs::Borrowed(connections),
                |sink| remaining.binary_search(&sink).is_ok(),
                |_| false,
            )
        });
        let Some(next) = next else { return Ok(None) };
        result.push(remaining.remove(next));
    }
    Ok(Some(result))
}
#[cfg(feature = "indexed-source")]
pub(crate) fn plain_startup_dependencies_ref<'a>(
    connections: &[conduit_core::PlannedConnectionRef<'a>],
) -> Result<
    alloc::vec::Vec<conduit_core::StartupDependencyRef<'a>>,
    alloc::collections::TryReserveError,
> {
    let mut dependencies = alloc::vec::Vec::new();
    dependencies.try_reserve_exact(connections.len())?;
    dependencies.extend(
        connections
            .iter()
            .filter(|c| dependency_required_ref(**c, false))
            .map(|c| conduit_core::StartupDependencyRef {
                prerequisite_placement_id: c.sink_placement_id,
                dependent_placement_id: c.source_placement_id,
            }),
    );
    dependencies.sort_unstable_by_key(|d| (d.prerequisite_placement_id, d.dependent_placement_id));
    dependencies.dedup_by_key(|d| (d.prerequisite_placement_id, d.dependent_placement_id));
    Ok(dependencies)
}

fn dependency_required_ref(
    connection: conduit_core::PlannedConnectionRef<'_>,
    delayed: bool,
) -> bool {
    connection.source_placement_id != connection.sink_placement_id && !delayed
}

fn candidate_dependency_allows_ref(
    candidate: &str,
    connection: conduit_core::PlannedConnectionRef<'_>,
    delayed: bool,
    sink_remaining: bool,
) -> bool {
    connection.source_placement_id == connection.sink_placement_id
        || delayed
        || connection.source_placement_id != candidate
        || !sink_remaining
}
