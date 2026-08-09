use alloc::vec::Vec;

use conduit_core::{
    AdmittedLine, ClueId, LineAvailability, LineAvailabilitySign, LineId, PlannedConnection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineCandidateState {
    line: AdmittedLine,
    availability: LineAvailability,
    sign_id: Option<ClueId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LineError {
    NoSealedCandidates,
    DuplicateCandidate,
    UnsealedObservation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LineDisposition<'a> {
    Selected {
        line: &'a AdmittedLine,
        same_plan_continues: bool,
    },
    Unsatisfied {
        replan_may_be_requested: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LineUpdate<'a> {
    pub sign: &'a LineAvailabilitySign,
    pub previous_selection: Option<&'a LineId>,
    pub disposition: LineDisposition<'a>,
}

/// Deterministic selection over one immutable, ordered, Plan-sealed Line set.
///
/// Signs may change availability, but cannot add candidates, reorder
/// policy, mutate the Plan, or invoke a planner. The first Ready candidate wins.
pub struct LineMachine {
    candidates: Vec<LineCandidateState>,
    selected: Option<usize>,
}

impl LineMachine {
    pub fn new(connection: &PlannedConnection) -> Result<Self, LineError> {
        let lines = connection.admitted_lines.clone();
        if lines.is_empty() {
            return Err(LineError::NoSealedCandidates);
        }
        for (index, line) in lines.iter().enumerate() {
            if lines[..index]
                .iter()
                .any(|prior| prior.line_id == line.line_id)
            {
                return Err(LineError::DuplicateCandidate);
            }
        }
        let selected = connection.selected_line.as_ref();
        let candidates = lines
            .into_iter()
            .map(|line| {
                let availability = selected
                    .filter(|selected| **selected == line)
                    .map_or(LineAvailability::Unavailable, |_| LineAvailability::Ready);
                LineCandidateState {
                    line,
                    availability,
                    sign_id: None,
                }
            })
            .collect();
        let mut machine = Self {
            candidates,
            selected: None,
        };
        machine.select_first_ready();
        Ok(machine)
    }

    pub fn selected(&self) -> Option<&AdmittedLine> {
        self.selected.map(|index| &self.candidates[index].line)
    }

    /// Return the exact selected Plan-sealed Line for session attachment.
    pub fn selected_line(&self) -> Option<AdmittedLine> {
        self.selected().cloned()
    }

    pub fn disposition(&self) -> LineDisposition<'_> {
        match self.selected() {
            Some(line) => LineDisposition::Selected {
                line,
                same_plan_continues: true,
            },
            None => LineDisposition::Unsatisfied {
                replan_may_be_requested: true,
            },
        }
    }

    pub fn observe<'a>(
        &'a mut self,
        sign: &'a LineAvailabilitySign,
    ) -> Result<LineUpdate<'a>, LineError> {
        let previous_index = self.selected;
        let candidate = self
            .candidates
            .iter_mut()
            .find(|candidate| {
                candidate.line.line_id == sign.line_id
                    && candidate.line.binding.binding_id == sign.binding_id
            })
            .ok_or(LineError::UnsealedObservation)?;
        candidate.availability = sign.availability;
        candidate.sign_id = Some(sign.sign_id.clone());
        self.select_first_ready();
        Ok(LineUpdate {
            sign,
            previous_selection: previous_index.map(|index| &self.candidates[index].line.line_id),
            disposition: self.disposition(),
        })
    }

    fn select_first_ready(&mut self) {
        self.selected = self
            .candidates
            .iter()
            .position(|candidate| candidate.availability == LineAvailability::Ready);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        AdmittedLine, BootId, BoundLink, ConnectionBase, ConnectionBaseInstanceId, ConnectionId,
        HostId, KindId, LineContinuation, LineContract, LineDuplex, LineOrdering, LineReliability,
        LineScope, LineSecurity, LineTrafficShape, LinkAuthorityReference, LinkBindingId,
        LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, PlacementId, PortId,
        PortTemporal,
    };

    fn line(id: &'static str, base: ConnectionBase) -> AdmittedLine {
        AdmittedLine {
            line_id: LineId::from(alloc::format!("line/{id}")),
            binding: BoundLink {
                binding_id: LinkBindingId::from(alloc::format!("binding/{id}")),
                source: LinkEndpoint {
                    host_id: HostId::from("source"),
                    boot_id: BootId::from("source-boot"),
                    endpoint_id: LinkEndpointId::from(alloc::format!("{id}/source")),
                },
                sink: LinkEndpoint {
                    host_id: HostId::from("sink"),
                    boot_id: BootId::from("sink-boot"),
                    endpoint_id: LinkEndpointId::from(alloc::format!("{id}/sink")),
                },
                base,
                base_instance_id: ConnectionBaseInstanceId::from(alloc::format!("{id}/base")),
                credential: LinkCredentialReference::None,
                authority: LinkAuthorityReference::ProcessOwned,
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 64,
                    maximum_buffered_bytes: 64,
                    maximum_frame_bytes: 1024,
                },
            },
            contract: LineContract {
                scope: LineScope::Machine,
                traffic_shape: LineTrafficShape::Message,
                duplex: LineDuplex::FullDuplex,
                ordering: LineOrdering::Ordered,
                reliability: LineReliability::Reliable,
                continuation: LineContinuation::None,
                security: LineSecurity::PlaintextNetwork,
            },
        }
    }

    fn connection(lines: &[AdmittedLine]) -> PlannedConnection {
        PlannedConnection {
            connection_id: ConnectionId::from("connection"),
            source_placement_id: PlacementId::from("source-placement"),
            source_port_id: PortId::from("out"),
            sink_placement_id: PlacementId::from("sink-placement"),
            sink_port_id: PortId::from("in"),
            value_kind: KindId::from("value/test@1"),
            temporal: PortTemporal::Value,
            selected_line: Some(lines[0].clone()),
            admitted_lines: lines.to_vec(),
            item_capacity: 1,
            byte_capacity: 64,
        }
    }

    fn observation(
        id: &'static str,
        availability: LineAvailability,
        clue: &'static str,
    ) -> LineAvailabilitySign {
        LineAvailabilitySign {
            line_id: LineId::from(alloc::format!("line/{id}")),
            binding_id: LinkBindingId::from(alloc::format!("binding/{id}")),
            availability,
            sign_id: ClueId::from(clue),
        }
    }

    #[test]
    fn ordered_selection_switches_only_within_the_sealed_set() {
        let usb = line("usb", ConnectionBase::UsbCdc);
        let ws = line("ws", ConnectionBase::WebSocket);
        let mut machine = LineMachine::new(&connection(&[usb, ws])).unwrap();

        assert_eq!(machine.selected().unwrap().line_id.as_str(), "line/usb");
        let ws_ready = observation("ws", LineAvailability::Ready, "ws-ready");
        machine.observe(&ws_ready).unwrap();
        let changed = observation("usb", LineAvailability::Unavailable, "usb-lost");
        let update = machine.observe(&changed).unwrap();
        assert_eq!(update.previous_selection.unwrap().as_str(), "line/usb");
        assert!(matches!(
            update.disposition,
            LineDisposition::Selected {
                line,
                same_plan_continues: true
            } if line.line_id.as_str() == "line/ws"
        ));
        assert_eq!(machine.selected_line().as_ref(), machine.selected());

        let invented = observation("invented", LineAvailability::Ready, "ambient");
        assert_eq!(
            machine.observe(&invented),
            Err(LineError::UnsealedObservation)
        );
        assert_eq!(machine.selected().unwrap().line_id.as_str(), "line/ws");
    }

    #[test]
    fn exhausted_single_and_multi_route_plans_are_explicitly_unsatisfied() {
        let usb = line("usb", ConnectionBase::UsbCdc);
        let mut one = LineMachine::new(&connection(core::slice::from_ref(&usb))).unwrap();
        let lost = observation("usb", LineAvailability::Unavailable, "usb-lost");
        assert!(matches!(
            one.observe(&lost).unwrap().disposition,
            LineDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ));

        let ws = line("ws", ConnectionBase::WebSocket);
        let mut two = LineMachine::new(&connection(&[usb, ws])).unwrap();
        two.observe(&lost).unwrap();
        let ws_lost = observation("ws", LineAvailability::Unavailable, "ws-lost");
        assert!(matches!(
            two.observe(&ws_lost).unwrap().disposition,
            LineDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ));
    }
}
