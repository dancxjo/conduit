//! Ordinary Cord, queue, quantity and resource admission after placement.
use super::*;

pub(super) fn plan(
    plot: &PlanningInput<'_>,
    planned_gears: &[PlannedGear],
    placement_lookup: &BTreeMap<GearId, PlacementId>,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    storage: Option<Vec<PlannedConnection>>,
) -> Result<Vec<PlannedConnection>, PlannerError> {
    let line_policy = contract::LineMechanismPolicy::new(bases);
    let PlanningOptions {
        connection_bases,
        line_candidates,
        connection_item_capacity,
        connection_byte_capacity,
        line_offers,
        ..
    } = options;
    let quantity_contracts = quantity_wire::contracts(planned_gears)?;
    let mut planned_connections =
        storage.unwrap_or_else(|| Vec::with_capacity(plot.connections.len()));
    for connection in plot.connections.iter() {
        quantity_wire::validate_connection(connection, &quantity_contracts)?;
        let limits = connection_limits
            .get(&connection_endpoints(connection))
            .copied()
            .unwrap_or(ConnectionQueueLimits {
                item_capacity: connection_item_capacity,
                byte_capacity: connection_byte_capacity,
            });
        quantity_wire::validate_capacity(connection, &quantity_contracts, limits.byte_capacity)?;
        let source_placement = placement_lookup
            .get(&connection.source_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.source_gear_id.as_str().to_string())
            })?;
        let sink_placement = placement_lookup
            .get(&connection.sink_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.sink_gear_id.as_str().to_string())
            })?;
        let source_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == source_placement)
            .expect("source placement must exist");
        let sink_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == sink_placement)
            .expect("sink placement must exist");
        let source_gear = plot
            .gears
            .connection_gear(&connection.source_gear_id)
            .expect("checked source gear must exist");
        let sink_gear = plot
            .gears
            .connection_gear(&connection.sink_gear_id)
            .expect("checked sink gear must exist");
        crate::value_storage::validate_capacity(
            connection,
            &source_gear,
            &sink_gear,
            limits.byte_capacity,
        )?;
        let resource = resource_port::plan_resource_connection(
            connection,
            &source_gear,
            &sink_gear,
            source_plan,
            sink_plan,
        )?;
        let (selected_line, admitted_lines) = select_line(LineSelection {
            source: source_plan,
            sink: sink_plan,
            policy: line_policy,
            requested: connection_bases
                .get(&(
                    connection.source_gear_id.clone(),
                    connection.sink_gear_id.clone(),
                ))
                .cloned(),
            requested_candidates: line_candidates.get(&(
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            )),
            line_offers,
            connection_item_capacity: limits.item_capacity,
            connection_byte_capacity: limits.byte_capacity,
        })?;
        if limits.item_capacity > source_plan.limits.max_queue_items
            || limits.item_capacity > sink_plan.limits.max_queue_items
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires item capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.item_capacity
            )));
        }
        if limits.byte_capacity > source_plan.limits.max_queue_bytes
            || limits.byte_capacity > sink_plan.limits.max_queue_bytes
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires byte capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.byte_capacity
            )));
        }
        planned_connections.push(PlannedConnection {
            connection_id: ConnectionId::from(hash_string(&format!(
                "connection:{}:{}:{}:{}:{}:{}:{}:{}",
                plot.checked_plot_id.as_str(),
                connection.source_gear_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_gear_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str(),
                connection.track.as_str(),
                connection.temporal.as_str(),
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            resource: resource.map(alloc::boxed::Box::new),
            abnormal_kind: source_plan
                .outputs
                .iter()
                .find(|port| port.port_id == connection.source_port_id)
                .and_then(|port| port.abnormal_kind.clone()),
            track: connection.track,
            temporal: connection.temporal,
            pressure_policy: if source_plan.kind_id.as_str() == "flow/coalesce-latest" {
                DeliveryPressurePolicy::CoalesceLatest
            } else {
                DeliveryPressurePolicy::PreserveOrder
            },
            selected_line: selected_line.map(alloc::boxed::Box::new),
            admitted_lines,
            item_capacity: limits.item_capacity,
            byte_capacity: limits.byte_capacity,
        });
    }

    Ok(planned_connections)
}
