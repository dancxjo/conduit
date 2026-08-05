use super::*;


pub(super) fn terminate_placement(
    placement: &mut RuntimePlacement,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if placement.terminal.is_some() {
        return;
    }
    placement.terminal = Some(disposition);
    placement.lifecycle = match disposition {
        TerminalDisposition::Completed => PlacementLifecycleState::Completed,
        TerminalDisposition::Failed { .. } => PlacementLifecycleState::Failed,
        TerminalDisposition::Cancelled { .. } => PlacementLifecycleState::Cancelled,
    };
    observations.push((
        Some(plan_id.clone()),
        Some(placement.spec.placement_id.clone()),
        None,
        ObservationKind::PlacementTerminal { disposition },
    ));
    events.push(HostEvent::PlacementTerminated {
        plan_id: plan_id.clone(),
        placement_id: placement.spec.placement_id.clone(),
        disposition,
    });
}

pub(super) fn terminate_connection(
    connection: &mut RuntimeConnection,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if connection.terminal.is_some() {
        return;
    }
    let report = ConnectionTerminalDisposition {
        disposition,
        last_accepted_sequence: connection.last_accepted_sequence,
        last_manifested_sequence: connection.last_manifested_sequence,
        undeliverable_items: connection
            .queue
            .len()
            .saturating_add(connection.accepted_remote_sequences.len())
            as u16,
    };
    connection.queued_bytes = 0;
    while connection.queue.pop().is_some() {}
    connection.accepted_remote_sequences.clear();
    connection.terminal = Some(report.clone());
    observations.push((
        Some(plan_id.clone()),
        None,
        Some(connection.spec.connection_id.clone()),
        ObservationKind::ConnectionTerminal {
            disposition: report.clone(),
        },
    ));
    events.push(HostEvent::ConnectionTerminated {
        plan_id: plan_id.clone(),
        connection_id: connection.spec.connection_id.clone(),
        disposition: report,
    });
}

pub(super) fn fail_operation(
    plan: &mut RuntimePlan,
    placement_id: &PlacementId,
    failure: ImplementationFailure,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if let Some(placement) = plan.placements.get_mut(placement_id) {
        terminate_placement(
            placement,
            TerminalDisposition::Failed {
                reason: failure.reason,
            },
            observations,
            events,
            plan_id,
        );
    }
    for connection_id in incoming_connections(placement_id, &plan.connections) {
        if let Some(connection) = plan.connections.get_mut(&connection_id) {
            connection.sink_failed = true;
            terminate_connection(
                connection,
                TerminalDisposition::Failed {
                    reason: failure.reason,
                },
                observations,
                events,
                plan_id,
            );
        }
    }
    for connection_id in outgoing_connections_all(placement_id, &plan.connections) {
        if let Some(connection) = plan.connections.get_mut(&connection_id) {
            terminate_connection(
                connection,
                TerminalDisposition::Failed {
                    reason: failure.reason,
                },
                observations,
                events,
                plan_id,
            );
        }
    }
    terminate_composite_outputs_for_placement(
        plan,
        placement_id,
        TerminalDisposition::Failed {
            reason: failure.reason,
        },
    );
    observations.push((
        Some(plan_id.clone()),
        Some(placement_id.clone()),
        None,
        ObservationKind::Failure {
            reason: failure.reason,
            message: failure.message,
        },
    ));
    if plan.state != PlanState::Failed {
        plan.state = PlanState::Failed;
        plan.terminal = Some(TerminalDisposition::Failed {
            reason: FailureReason::RequiredBranchFailed,
        });
        cancel_active_sources(
            plan,
            CancellationReason::RequiredPlanFailed,
            observations,
            events,
            plan_id,
        );
    }
}

pub(super) fn cancel_active_sources(
    plan: &mut RuntimePlan,
    reason: CancellationReason,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    let placement_ids = plan.placements.keys().cloned().collect::<Vec<_>>();
    for placement_id in placement_ids {
        let Some(placement) = plan.placements.get_mut(&placement_id) else {
            continue;
        };
        if !placement.spec.outputs.is_empty()
            && placement.lifecycle == PlacementLifecycleState::Active
        {
            placement.implementation_state.cancel();
            terminate_placement(
                placement,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
            mark_source_done(&placement_id, &mut plan.connections);
            terminate_composite_outputs_for_placement(
                plan,
                &placement_id,
                TerminalDisposition::Cancelled { reason },
            );
        }
    }
}

pub(super) fn cancel_all_placements_and_connections(
    plan: &mut RuntimePlan,
    reason: CancellationReason,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    for placement in plan.placements.values_mut() {
        if placement.terminal.is_none() {
            placement.implementation_state.cancel();
            terminate_placement(
                placement,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
        }
        placement.action = OperationAction::Idle;
        placement.effect_issued = false;
        placement.pending_input_connection = None;
        placement.pending_input_boundary = None;
    }
    for connection in plan.connections.values_mut() {
        if connection.terminal.is_none() {
            terminate_connection(
                connection,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
        }
    }
    for input in plan.composite_inputs.values_mut() {
        input.closed = true;
        while input.queue.pop().is_some() {}
        input.queued_bytes = 0;
    }
    for output in plan.composite_outputs.values_mut() {
        while output.queue.pop().is_some() {}
        output.queued_bytes = 0;
        output.transmission_in_flight = false;
        output.terminal = Some(TerminalDisposition::Cancelled { reason });
    }
}

pub(super) fn terminate_composite_outputs_for_placement(
    plan: &mut RuntimePlan,
    placement_id: &PlacementId,
    disposition: TerminalDisposition,
) {
    for output in plan
        .composite_outputs
        .values_mut()
        .filter(|output| output.binding.placement_id == *placement_id)
    {
        while output.queue.pop().is_some() {}
        output.queued_bytes = 0;
        output.transmission_in_flight = false;
        output.terminal = Some(disposition);
    }
}

pub(super) fn outgoing_connections(
    placement_id: &PlacementId,
    port_id: &conduit_core::PortId,
    connections: &BTreeMap<ConnectionId, RuntimeConnection>,
) -> Vec<ConnectionId> {
    connections
        .iter()
        .filter_map(|(connection_id, connection)| {
            if &connection.spec.source_placement_id == placement_id
                && &connection.spec.source_port_id == port_id
            {
                Some(connection_id.clone())
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn outgoing_connections_all(
    placement_id: &PlacementId,
    connections: &BTreeMap<ConnectionId, RuntimeConnection>,
) -> Vec<ConnectionId> {
    connections
        .iter()
        .filter(|(_, connection)| &connection.spec.source_placement_id == placement_id)
        .map(|(connection_id, _)| connection_id.clone())
        .collect()
}

pub(super) fn incoming_connections(
    placement_id: &PlacementId,
    connections: &BTreeMap<ConnectionId, RuntimeConnection>,
) -> Vec<ConnectionId> {
    connections
        .iter()
        .filter_map(|(connection_id, connection)| {
            if &connection.spec.sink_placement_id == placement_id {
                Some(connection_id.clone())
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn mark_source_done(
    placement_id: &PlacementId,
    connections: &mut BTreeMap<ConnectionId, RuntimeConnection>,
) {
    for connection in connections.values_mut() {
        if &connection.spec.source_placement_id == placement_id {
            connection.source_done = true;
        }
    }
}
