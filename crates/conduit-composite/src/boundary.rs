use crate::child::BoundaryEndpoint;
use conduit_core::PortDirection;
use conduit_kernel::scheduler::{CordCapacity, CordSpec};
use conduit_kernel::{NodeId, PortId, RouteRange, RouteTarget};
use conduit_runtime::lowering::LoweredPlanFragment;
use std::collections::BTreeMap;

pub(crate) fn augment_boundary_cords(
    lowered: &mut LoweredPlanFragment,
    boundaries: &[BoundaryEndpoint],
) -> Result<(), String> {
    for boundary in boundaries {
        let identity = lowered
            .identity
            .ports
            .iter()
            .find(|identity| {
                identity.port_id == boundary.internal_port_id
                    && identity.direction == boundary.direction
            })
            .ok_or_else(|| {
                format!(
                    "boundary port '{}' is absent",
                    boundary.internal_port_id.as_str()
                )
            })?;
        let capacity = CordCapacity {
            slot_start: lowered.cord_value_slots,
            item_capacity: boundary.item_capacity,
            byte_capacity: boundary.byte_capacity,
        };
        let spec = match boundary.direction {
            PortDirection::Input => CordSpec::remote_ingress(
                boundary.cord,
                boundary.endpoint,
                (identity.node, identity.port),
                capacity,
            ),
            PortDirection::Output => CordSpec::remote_egress(
                boundary.cord,
                (identity.node, identity.port),
                boundary.endpoint,
                capacity,
            ),
        };
        lowered.cord_value_slots = lowered
            .cord_value_slots
            .checked_add(boundary.item_capacity)
            .ok_or_else(|| "boundary item capacity overflow".to_string())?;
        lowered.cord_value_bytes = lowered
            .cord_value_bytes
            .checked_add(boundary.byte_capacity)
            .ok_or_else(|| "boundary byte capacity overflow".to_string())?;
        lowered.cords.push(conduit_runtime::lowering::LoweredCord {
            connection_id: conduit_core::ConnectionId::from(format!(
                "composite-boundary/{}",
                boundary.external_port_id.as_str()
            )),
            spec,
        });
        if boundary.direction == PortDirection::Input {
            let slot = lowered
                .node_specs
                .get_mut(usize::from(identity.node.0))
                .and_then(|node| node.input_cords.get_mut(usize::from(identity.port.0)))
                .ok_or_else(|| "boundary input exceeds the lowered node shape".to_string())?;
            if slot.replace(boundary.cord).is_some() {
                return Err("composite input face already has an internal Cord".into());
            }
        }
    }
    rebuild_routes(lowered)
}

fn rebuild_routes(lowered: &mut LoweredPlanFragment) -> Result<(), String> {
    let mut grouped = BTreeMap::<(NodeId, PortId), Vec<RouteTarget>>::new();
    for cord in &lowered.cords {
        if let Some((node, port)) = cord.spec.source_local() {
            grouped.entry((node, port)).or_default().push(RouteTarget {
                cord: cord.spec.cord,
                sink: cord.spec.sink,
            });
        }
    }
    let mut cursor = 0u16;
    lowered.routes.clear();
    for ((source_node, source_port), targets) in grouped {
        let len = u16::try_from(targets.len()).map_err(debug)?;
        lowered
            .routes
            .push(conduit_runtime::lowering::LoweredRoute {
                source_node,
                source_port,
                range: RouteRange { start: cursor, len },
                targets,
            });
        cursor = cursor
            .checked_add(len)
            .ok_or_else(|| "boundary route capacity overflow".to_string())?;
    }
    Ok(())
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
