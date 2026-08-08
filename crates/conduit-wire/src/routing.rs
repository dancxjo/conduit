use alloc::vec::Vec;

use conduit_core::{
    BoundLink, EvidenceId, LinkAvailability, LinkBinding, LinkBindingId, LinkObservation,
    PlannedConnection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteCandidateState {
    link: BoundLink,
    availability: LinkAvailability,
    evidence_id: Option<EvidenceId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RouteError {
    NoSealedCandidates,
    DuplicateCandidate,
    UnsealedObservation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RouteDisposition<'a> {
    Selected {
        link: &'a BoundLink,
        same_plan_continues: bool,
    },
    Unsatisfied {
        replan_may_be_requested: bool,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RouteUpdate<'a> {
    pub observation: &'a LinkObservation,
    pub previous_selection: Option<&'a LinkBindingId>,
    pub disposition: RouteDisposition<'a>,
}

/// Deterministic selection over one immutable, ordered, Plan-sealed route set.
///
/// Observations may change availability, but cannot add candidates, reorder
/// policy, mutate the Plan, or invoke a planner. The first Ready candidate wins.
pub struct RouteMachine {
    candidates: Vec<RouteCandidateState>,
    selected: Option<usize>,
}

impl RouteMachine {
    pub fn new(connection: &PlannedConnection) -> Result<Self, RouteError> {
        let links = if connection.route_candidates.is_empty() {
            connection
                .link_binding
                .as_ref()
                .map(|link| alloc::vec![link.bound_link()])
                .unwrap_or_default()
        } else {
            connection.route_candidates.clone()
        };
        if links.is_empty() {
            return Err(RouteError::NoSealedCandidates);
        }
        for (index, link) in links.iter().enumerate() {
            if links[..index]
                .iter()
                .any(|prior| prior.binding_id == link.binding_id)
            {
                return Err(RouteError::DuplicateCandidate);
            }
        }
        let legacy_ready = connection.link_binding.as_ref();
        let candidates = links
            .into_iter()
            .map(|link| {
                let availability = legacy_ready
                    .filter(|observed| observed.bound_link() == link)
                    .map_or(LinkAvailability::Unavailable, |observed| {
                        observed.availability
                    });
                RouteCandidateState {
                    link,
                    availability,
                    evidence_id: None,
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

    pub fn selected(&self) -> Option<&BoundLink> {
        self.selected.map(|index| &self.candidates[index].link)
    }

    /// Combine the selected Plan-sealed identity with its current mutable
    /// observation for exact attachment admission.
    pub fn selected_binding(&self) -> Option<LinkBinding> {
        let candidate = &self.candidates[self.selected?];
        Some(LinkBinding {
            binding_id: candidate.link.binding_id.clone(),
            source: candidate.link.source.clone(),
            sink: candidate.link.sink.clone(),
            provider: candidate.link.provider,
            provider_instance_id: candidate.link.provider_instance_id.clone(),
            availability: candidate.availability,
            credential: candidate.link.credential.clone(),
            authority: candidate.link.authority.clone(),
            limits: candidate.link.limits,
        })
    }

    pub fn disposition(&self) -> RouteDisposition<'_> {
        match self.selected() {
            Some(link) => RouteDisposition::Selected {
                link,
                same_plan_continues: true,
            },
            None => RouteDisposition::Unsatisfied {
                replan_may_be_requested: true,
            },
        }
    }

    pub fn observe<'a>(
        &'a mut self,
        observation: &'a LinkObservation,
    ) -> Result<RouteUpdate<'a>, RouteError> {
        let previous_index = self.selected;
        let candidate = self
            .candidates
            .iter_mut()
            .find(|candidate| candidate.link.binding_id == observation.binding_id)
            .ok_or(RouteError::UnsealedObservation)?;
        candidate.availability = observation.availability;
        candidate.evidence_id = Some(observation.evidence_id.clone());
        self.select_first_ready();
        Ok(RouteUpdate {
            observation,
            previous_selection: previous_index.map(|index| &self.candidates[index].link.binding_id),
            disposition: self.disposition(),
        })
    }

    fn select_first_ready(&mut self) {
        self.selected = self
            .candidates
            .iter()
            .position(|candidate| candidate.availability == LinkAvailability::Ready);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId, HostId, KindId,
        LinkAuthorityReference, LinkBinding, LinkCredentialReference, LinkEndpoint, LinkEndpointId,
        LinkLimits, PlacementId, PortId, PortTemporal,
    };

    fn link(id: &'static str, provider: ConnectionProvider) -> LinkBinding {
        LinkBinding {
            binding_id: LinkBindingId::from(id),
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
            provider,
            provider_instance_id: ConnectionProviderInstanceId::from(alloc::format!(
                "{id}/provider"
            )),
            availability: LinkAvailability::Ready,
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 1024,
            },
        }
    }

    fn connection(links: &[LinkBinding]) -> PlannedConnection {
        PlannedConnection {
            connection_id: ConnectionId::from("connection"),
            source_placement_id: PlacementId::from("source-placement"),
            source_port_id: PortId::from("out"),
            sink_placement_id: PlacementId::from("sink-placement"),
            sink_port_id: PortId::from("in"),
            value_kind: KindId::from("value/test@1"),
            temporal: PortTemporal::Value,
            provider: links[0].provider,
            link_binding: Some(links[0].clone()),
            route_candidates: links.iter().map(LinkBinding::bound_link).collect(),
            item_capacity: 1,
            byte_capacity: 64,
        }
    }

    fn observation(
        binding_id: &'static str,
        availability: LinkAvailability,
        evidence: &'static str,
    ) -> LinkObservation {
        LinkObservation {
            binding_id: LinkBindingId::from(binding_id),
            availability,
            evidence_id: EvidenceId::from(evidence),
        }
    }

    #[test]
    fn ordered_selection_switches_only_within_the_sealed_set() {
        let usb = link("usb", ConnectionProvider::UsbCdc);
        let ws = link("ws", ConnectionProvider::WebSocket);
        let mut machine = RouteMachine::new(&connection(&[usb, ws])).unwrap();

        assert_eq!(machine.selected().unwrap().binding_id.as_str(), "usb");
        let ws_ready = observation("ws", LinkAvailability::Ready, "ws-ready");
        machine.observe(&ws_ready).unwrap();
        let changed = observation("usb", LinkAvailability::Unavailable, "usb-lost");
        let update = machine.observe(&changed).unwrap();
        assert_eq!(update.previous_selection.unwrap().as_str(), "usb");
        assert!(matches!(
            update.disposition,
            RouteDisposition::Selected {
                link,
                same_plan_continues: true
            } if link.binding_id.as_str() == "ws"
        ));
        assert_eq!(
            machine.selected_binding().unwrap().bound_link(),
            machine.selected().unwrap().clone()
        );

        let invented = observation("invented", LinkAvailability::Ready, "ambient");
        assert_eq!(
            machine.observe(&invented),
            Err(RouteError::UnsealedObservation)
        );
        assert_eq!(machine.selected().unwrap().binding_id.as_str(), "ws");
    }

    #[test]
    fn exhausted_single_and_multi_route_plans_are_explicitly_unsatisfied() {
        let usb = link("usb", ConnectionProvider::UsbCdc);
        let mut one = RouteMachine::new(&connection(core::slice::from_ref(&usb))).unwrap();
        let lost = observation("usb", LinkAvailability::Unavailable, "usb-lost");
        assert!(matches!(
            one.observe(&lost).unwrap().disposition,
            RouteDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ));

        let ws = link("ws", ConnectionProvider::WebSocket);
        let mut two = RouteMachine::new(&connection(&[usb, ws])).unwrap();
        two.observe(&lost).unwrap();
        let ws_lost = observation("ws", LinkAvailability::Unavailable, "ws-lost");
        assert!(matches!(
            two.observe(&ws_lost).unwrap().disposition,
            RouteDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ));
    }
}
