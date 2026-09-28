//! Exact typed directional port numbering before Play.
use super::{as_u16, LoweredPort, LoweringError};
use alloc::{collections::BTreeSet, vec::Vec};
use conduit_core::{
    FrontValueContract, FrontValueLocation, PlacementId, PortDescriptor, PortDirection,
    PortId as PlanPortId,
};
use conduit_kernel::{NodeId, PortId};
pub(super) fn lower_ports(
    node: NodeId,
    placement_id: &PlacementId,
    ports: &[PortDescriptor],
    expected_direction: PortDirection,
    value_contracts: &[FrontValueContract],
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
                abnormal_kind: descriptor.abnormal_kind.clone(),
                maximum_value_bytes: value_contracts
                    .iter()
                    .find(|bound| {
                        bound.location
                            == match expected_direction {
                                PortDirection::Input => {
                                    FrontValueLocation::Input(descriptor.port_id.clone())
                                }
                                PortDirection::Output => {
                                    FrontValueLocation::Output(descriptor.port_id.clone())
                                }
                            }
                    })
                    .map(|contract| u64::from(contract.contract.maximum_bytes)),
                maximum_abnormal_value_bytes: value_contracts
                    .iter()
                    .find(|bound| {
                        bound.location
                            == match expected_direction {
                                PortDirection::Input => {
                                    FrontValueLocation::InputAbnormal(descriptor.port_id.clone())
                                }
                                PortDirection::Output => {
                                    FrontValueLocation::OutputAbnormal(descriptor.port_id.clone())
                                }
                            }
                    })
                    .map(|contract| u64::from(contract.contract.maximum_bytes)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::{kind_id, port_id, CheckedValueContract, PortTemporal};

    #[test]
    fn lowering_keeps_payload_and_abnormal_bounds_independent() {
        let descriptor = PortDescriptor {
            port_id: port_id("work"),
            value_kind: kind_id("value/bytes"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id("value/text")),
        };
        let contracts = vec![
            FrontValueContract {
                location: FrontValueLocation::Output(port_id("work")),
                contract: CheckedValueContract::new(kind_id("value/bytes"), 4_096, vec![]).unwrap(),
            },
            FrontValueContract {
                location: FrontValueLocation::OutputAbnormal(port_id("work")),
                contract: CheckedValueContract::new(kind_id("value/text"), 37, vec![]).unwrap(),
            },
        ];

        let lowered = lower_ports(
            NodeId(0),
            &PlacementId::from("worker"),
            &[descriptor],
            PortDirection::Output,
            &contracts,
        )
        .unwrap();
        assert_eq!(lowered[0].maximum_value_bytes, Some(4_096));
        assert_eq!(lowered[0].maximum_abnormal_value_bytes, Some(37));
    }
}
