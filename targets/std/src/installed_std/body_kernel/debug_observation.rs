//! Exact observation bindings prepared from the same lowered Body partitions.
use super::super::{debug_sign_sink::InstalledDebugScope, MAX_CORDS, MAX_NODES};
use conduit_core::PlanFragment;
use conduit_kernel::debug_observation::{DebugExecutionIdentity, DebugNodeBinding};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn prepare(
    execution: DebugExecutionIdentity,
    fragments: &[&PlanFragment],
    partitions: &[LoweredPlanFragment],
) -> Result<(InstalledDebugScope, Vec<String>), String> {
    if fragments.len() != partitions.len() {
        return Err("debug observation partition extent differs".into());
    }
    // The existing observer correlates the latest send on each Cord. A
    // one-slot Cord cannot queue a later send ahead of an unread earlier value.
    // Wider queues require FIFO correlation; never manufacture their parents.
    if partitions
        .iter()
        .flat_map(|part| &part.cords)
        .any(|cord| cord.spec.item_capacity > 1)
    {
        return Err(
            "debug observation requires one-slot Cords; FIFO parent correlation is not admitted"
                .into(),
        );
    }
    let mut scope = InstalledDebugScope {
        execution,
        node_bindings: [DebugNodeBinding { plot: 0, host: 0 }; MAX_NODES],
        cord_types: [None; MAX_CORDS],
    };
    let mut type_names = Vec::<String>::with_capacity(MAX_CORDS);
    for (index, (fragment, partition)) in fragments.iter().zip(partitions).enumerate() {
        let plot = u16::try_from(index).map_err(|_| "debug Plot binding exceeds bound")?;
        for node in &partition.nodes {
            *scope
                .node_bindings
                .get_mut(usize::from(node.node.0))
                .ok_or("debug node binding exceeds installed bound")? =
                DebugNodeBinding { plot, host: 0 };
        }
        for cord in &partition.cords {
            let kind = fragment
                .connections
                .iter()
                .find(|connection| connection.connection_id == cord.connection_id)
                .map(|connection| &connection.value_kind)
                .or_else(|| {
                    partition
                        .identity
                        .fore_endpoints
                        .iter()
                        .find(|endpoint| endpoint.cord == cord.spec.cord)
                        .map(|endpoint| &endpoint.value_kind)
                })
                .or_else(|| {
                    partition
                        .remote_endpoints
                        .iter()
                        .find(|endpoint| endpoint.cord == cord.spec.cord)
                        .map(|endpoint| &endpoint.value_kind)
                })
                .ok_or("debug Cord lacks its exact admitted value kind")?;
            let index = match type_names.iter().position(|name| name == kind.as_str()) {
                Some(index) => index,
                None => {
                    type_names.push(kind.as_str().into());
                    type_names.len() - 1
                }
            };
            *scope
                .cord_types
                .get_mut(usize::from(cord.spec.cord.0))
                .ok_or("debug Cord binding exceeds installed bound")? =
                Some(u16::try_from(index).map_err(|_| "debug type binding exceeds bound")?);
        }
    }
    Ok((scope, type_names))
}

/// Detach only after terminal cleanup. Export allocation is outside Play.
pub(super) fn finish(
    scheduler: &mut super::super::InstalledScheduler,
    types: Option<(DebugExecutionIdentity, Vec<String>)>,
) -> Option<crate::body_debug_observation::BodyDebugHistory> {
    types.map(
        |(execution, type_names)| match scheduler.detach_debug_observer() {
            Ok(buffer) => crate::body_debug_observation::BodyDebugHistory {
                execution,
                records: (0..buffer.len())
                    .filter_map(|index| buffer.record(index).copied())
                    .collect(),
                gap: buffer.gap(),
                type_names,
                refusal: None,
            },
            Err(refusal) => crate::body_debug_observation::BodyDebugHistory {
                execution,
                records: Vec::new(),
                gap: None,
                type_names,
                refusal: Some(refusal),
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wider_queue_refuses_observation_instead_of_inventing_latest_send_parent() {
        let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap();
        let mut lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        lowered.cords[0].spec.item_capacity = 2;
        let execution = DebugExecutionIdentity {
            body: [1; 32],
            plan: [2; 32],
            play: [3; 32],
        };
        let error = prepare(execution, &[fragment], &[lowered]).err().unwrap();
        assert!(error.contains("FIFO parent correlation"));
    }
}
