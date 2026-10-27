//! Verification of exact external Fore contracts, split terminal exports and Lines.
use crate::*;

pub(crate) fn verify_fragment_fore_ports(fragment: &PlanFragment) -> bool {
    fragment
        .fore_ports
        .iter()
        .enumerate()
        .all(|(index, boundary)| {
            boundary.item_capacity > 0
                && boundary.byte_capacity > 0
                && boundary
                    .selected_line
                    .as_ref()
                    .is_none_or(|line| valid_fore_line(fragment, boundary, line))
                && !fragment.fore_ports[..index].iter().any(|prior| {
                    let same_fore = prior.front_port_id == boundary.front_port_id
                        && prior.direction == boundary.direction
                        && prior.track == boundary.track;
                    same_fore
                        && (boundary.direction != PortDirection::Input
                            || boundary.track != ConnectionTrack::Payload
                            || (prior.placement_id == boundary.placement_id
                                && prior.gear_port_id == boundary.gear_port_id))
                })
                && fragment.placements.iter().any(|placement| {
                    placement.placement_id == boundary.placement_id
                        && match boundary.direction {
                            PortDirection::Input => &placement.inputs,
                            PortDirection::Output => &placement.outputs,
                        }
                        .iter()
                        .any(|port| {
                            if port.port_id != boundary.gear_port_id {
                                return false;
                            }
                            let port_contract_matches =
                                match boundary.track {
                                    ConnectionTrack::Payload => {
                                        port.value_kind == boundary.value_kind
                                    && (port.abnormal_kind == boundary.abnormal_kind
                                        || (boundary.direction == PortDirection::Output
                                            && boundary.abnormal_kind.is_none()
                                            && port.abnormal_kind.as_ref().is_some_and(|kind| {
                                                // A split success export remains valid only
                                                // with the same Port's typed terminal export.
                                                fragment.fore_ports.iter().any(|terminal| {
                                                    terminal.direction == PortDirection::Output
                                                        && terminal.placement_id
                                                            == boundary.placement_id
                                                        && terminal.gear_port_id
                                                            == boundary.gear_port_id
                                                        && terminal.track
                                                            == ConnectionTrack::AbnormalTerminal
                                                        && &terminal.value_kind == kind
                                                        && terminal.temporal == PortTemporal::Value
                                                        && terminal.abnormal_kind.is_none()
                                                })
                                            })))
                                    && port.temporal == boundary.temporal
                                    }
                                    ConnectionTrack::AbnormalTerminal => {
                                        port.abnormal_kind.as_ref() == Some(&boundary.value_kind)
                                            && boundary.temporal == PortTemporal::Value
                                    }
                                    ConnectionTrack::NormalClose => {
                                        port.temporal == (PortTemporal::Flow { closes: true })
                                            && boundary.value_kind.as_str() == EMPTY_INFO_ID
                                            && boundary.temporal == PortTemporal::Value
                                    }
                                    ConnectionTrack::Quiescence => {
                                        matches!(port.temporal, PortTemporal::Flow { .. })
                                            && boundary.value_kind.as_str() == EMPTY_INFO_ID
                                            && boundary.temporal == PortTemporal::Value
                                    }
                                };
                            if !port_contract_matches {
                                return false;
                            }
                            let location = match (boundary.direction, boundary.track) {
                                (PortDirection::Input, ConnectionTrack::AbnormalTerminal) => {
                                    FrontValueLocation::InputAbnormal(port.port_id.clone())
                                }
                                (PortDirection::Output, ConnectionTrack::AbnormalTerminal) => {
                                    FrontValueLocation::OutputAbnormal(port.port_id.clone())
                                }
                                (PortDirection::Input, _) => {
                                    FrontValueLocation::Input(port.port_id.clone())
                                }
                                (PortDirection::Output, _) => {
                                    FrontValueLocation::Output(port.port_id.clone())
                                }
                            };
                            let internal_contract = placement
                                .checked_port_front()
                                .value_contract(&location)
                                .cloned();
                            boundary.track == ConnectionTrack::NormalClose
                                || internal_contract == boundary.value_contract
                        })
                })
        })
        && verify_fore_line_capacity(fragment.fore_ports.iter())
}

fn valid_fore_line(fragment: &PlanFragment, fore: &PlannedForePort, line: &AdmittedLine) -> bool {
    let binding = &line.binding;
    let (local, peer) = match fore.direction {
        PortDirection::Input => (&binding.sink, &binding.source),
        PortDirection::Output => (&binding.source, &binding.sink),
    };
    !line.line_id.as_str().is_empty()
        && !binding.binding_id.as_str().is_empty()
        && binding.base.as_str() != LOCAL_BASE_IMPLEMENTATION_ID
        && line.contract.scope != LineScope::Process
        && line.contract.traffic_shape == LineTrafficShape::Message
        && line.contract.ordering == LineOrdering::Ordered
        && line.contract.reliability == LineReliability::Reliable
        && !binding.base_instance_id.as_str().is_empty()
        && !local.endpoint_id.as_str().is_empty()
        && !peer.endpoint_id.as_str().is_empty()
        && local.host_id == fragment.host_id
        && local.boot_id == fragment.boot_id
        && peer.host_id != fragment.host_id
        && !peer.host_id.as_str().is_empty()
        && !peer.boot_id.as_str().is_empty()
        && binding.limits.maximum_in_flight_items >= fore.item_capacity
        && binding.limits.maximum_payload_bytes >= fore.byte_capacity
        && binding.limits.maximum_buffered_bytes >= fore.byte_capacity
        && binding.limits.maximum_frame_bytes >= binding.limits.maximum_payload_bytes
        && !matches!(&binding.credential, LinkCredentialReference::Opaque(id) if id.as_str().is_empty())
        && !matches!(&binding.authority, LinkAuthorityReference::Grant(id) if id.as_str().is_empty())
}
