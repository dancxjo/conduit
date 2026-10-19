//! Exact topology and mandatory evidence byte admission before execution.
use super::*;

pub(super) fn validate_plan<
    const NODES: usize,
    const CORDS: usize,
    const PORTS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
    E: SignSink,
>(
    active_nodes: usize,
    active_cords: usize,
    nodes: &[NodeSpec<PORTS>; NODES],
    cords: &[CordSpec; CORDS],
    routes: &FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    signs: &E,
) -> Result<(), SchedulerError> {
    // Every cord has one exact source, so membership is global across routes.
    let mut membership = None::<route_membership::RouteMembership<CORDS>>;
    for (node_index, node) in nodes[..active_nodes].iter().enumerate() {
        if node.maximum_step_fuel == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        for (port, cord) in node.input_cords.iter().copied().enumerate() {
            let Some(cord) = cord else {
                continue;
            };
            let spec = cords
                .get(usize::from(cord.0))
                .filter(|_| usize::from(cord.0) < active_cords)
                .ok_or(SchedulerError::InvalidPlan)?;
            if spec.sink_local() != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?))) {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for port in 0..PORTS {
            let Ok(targets) = route_membership::targets(
                routes,
                NodeId(as_u16(node_index)?),
                PortId(as_u16(port)?),
            ) else {
                continue;
            };
            for target in targets {
                let cord = usize::from(target.cord.0);
                let spec = cords
                    .get(cord)
                    .filter(|_| cord < active_cords)
                    .ok_or(SchedulerError::InvalidPlan)?;
                let seen = membership.get_or_insert_with(route_membership::RouteMembership::new);
                if seen.contains(cord)
                    || spec.source_local()
                        != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?)))
                    || spec.sink != target.sink
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                seen.mark(cord);
            }
        }
    }
    let mut prefix = cord_prefix::CordPrefix::default();
    for (cord_index, cord) in cords[..active_cords].iter().copied().enumerate() {
        if usize::from(cord.cord.0) != cord_index
            || cord.item_capacity == 0
            || cord.byte_capacity == 0
            || cord.maximum_value_bytes == 0
            || cord.maximum_value_bytes > cord.byte_capacity
        {
            return Err(SchedulerError::InvalidPlan);
        }
        match (cord.source, cord.sink) {
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(source_node.0) >= active_nodes
                    || usize::from(sink_node.0) >= active_nodes
                    || usize::from(source_port.0) >= PORTS
                    || usize::from(sink_port.0) >= PORTS
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Remote(_),
            ) => {
                if usize::from(source_node.0) >= active_nodes || usize::from(source_port.0) >= PORTS
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Remote(_),
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(sink_node.0) >= active_nodes || usize::from(sink_port.0) >= PORTS {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (CordEndpoint::Remote(_), CordEndpoint::Remote(_)) => {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        let end = usize::from(cord.slot_start)
            .checked_add(usize::from(cord.item_capacity))
            .ok_or(SchedulerError::InvalidPlan)?;
        if end > QUEUE_SLOTS {
            return Err(SchedulerError::InvalidPlan);
        }
        if let Some((sink_node, sink_port)) = cord.sink_local() {
            if nodes[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)]
                != Some(cord.cord)
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        if let Some((source_node, source_port)) = cord.source_local() {
            if !membership
                .as_ref()
                .is_some_and(|seen| seen.contains(cord_index))
            {
                // Keep RouteMissing distinct from an installed route that omits
                // this cord. A present membership already proves exact endpoints.
                let _ = route_membership::targets(routes, source_node, source_port)?;
                return Err(SchedulerError::InvalidPlan);
            }
        }
        prefix.admit(cord, &cords[..cord_index], end)?;
        signs.ensure_value_extent(cord.maximum_value_bytes)?;
    }
    Ok(())
}
