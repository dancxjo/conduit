//! Exact typed directional port numbering before Play.
use super::{as_u16, LoweredPort, LoweringError};
use alloc::{collections::BTreeSet, vec::Vec};
use conduit_core::{PlacementId, PortDescriptor, PortDirection, PortId as PlanPortId};
use conduit_kernel::{NodeId, PortId};
pub(super) fn lower_ports(
    node: NodeId,
    placement_id: &PlacementId,
    ports: &[PortDescriptor],
    expected_direction: PortDirection,
) -> Result<Vec<LoweredPort>, LoweringError> {
    let mut ids = BTreeSet::new();
    ports
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            if descriptor.direction != expected_direction {
                return Err(LoweringError::PortDirectionMismatch {
                    placement_id: placement_id.clone(),
                    port_id: descriptor.port_id.clone(),
                });
            }
            if !ids.insert(descriptor.port_id.clone()) {
                return Err(LoweringError::DuplicatePort {
                    placement_id: placement_id.clone(),
                    port_id: descriptor.port_id.clone(),
                });
            }
            Ok(LoweredPort {
                node,
                port: PortId(as_u16(index)?),
                port_id: descriptor.port_id.clone(),
                value_kind: descriptor.value_kind.clone(),
                direction: descriptor.direction,
                temporal: descriptor.temporal,
            })
        })
        .collect()
}

pub(super) fn find_port(ports: &[LoweredPort], id: &PlanPortId) -> Option<PortId> {
    ports
        .iter()
        .find(|port| &port.port_id == id)
        .map(|port| port.port)
}
