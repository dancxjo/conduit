//! Clue-backed projection of distributed route selection and replanning.

use conduit_core::{
    BootId, CapabilityId, ClueId, ConnectionBase, ControlLoopEvent, GearId, HostAdvertisement,
    HostId, LinkAvailability, LinkBindingId, LinkEndpointId, OfferGeneration,
    PlanningRequestAuthority, PlayUnsatisfiedReason,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_signal::{
    distributed_browser_sink_advertisement, distributed_websocket_link_binding,
    signal_profile_catalog, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use conduit_wire::{RouteDisposition, RouteError, RouteMachine};
use std::collections::BTreeMap;

use crate::route_presentation::{
    DistributedRoutePresentation, NewPlanRecoveryPresentation, RefusedRoutePresentation,
    RouteCandidatePresentation, RoutePlanPresentation, SamePlanFallbackPresentation,
};

const SOURCE: &str = include_str!("../../../examples/signal-demo.conduit");
const USB_ROUTE: &str = "s4/distributed-signal-usb-link";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDemoError {
    InvalidSemanticForm,
    Planning(String),
    MissingConnection,
    Route(RouteError),
    InvalidClue,
    InspectionTooLarge,
}

impl From<RouteError> for RouteDemoError {
    fn from(value: RouteError) -> Self {
        Self::Route(value)
    }
}

/// A finite document whose route facts are copied only from checked/planned
/// identities and validated runtime/control-loop records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedRouteDemo {
    lines: Vec<String>,
    presentation: DistributedRoutePresentation,
    control_events: Vec<ControlLoopEvent>,
}

impl DistributedRouteDemo {
    pub fn build() -> Result<Self, RouteDemoError> {
        Self::build_for_source(
            HostId::from("patchbay-native/std-realization"),
            BootId::from("patchbay-native/std-boot-1"),
        )
    }

    pub fn build_for_source(
        source_host_id: HostId,
        source_boot_id: BootId,
    ) -> Result<Self, RouteDemoError> {
        if SOURCE.contains("host")
            || SOURCE.contains("WebSocket")
            || SOURCE.contains("UsbCdc")
            || SOURCE.contains("USB")
        {
            return Err(RouteDemoError::InvalidSemanticForm);
        }
        let one = planned(&[USB_ROUTE], &source_host_id, &source_boot_id)?;
        let fallback = planned(
            &[USB_ROUTE, conduit_signal::DISTRIBUTED_LINK_BINDING_ID],
            &source_host_id,
            &source_boot_id,
        )?;
        let replacement = planned(
            &[conduit_signal::DISTRIBUTED_LINK_BINDING_ID],
            &source_host_id,
            &source_boot_id,
        )?;
        let one_connection = remote_connection(&one)?;
        let fallback_connection = remote_connection(&fallback)?;

        let mut lines = Vec::with_capacity(64);
        push(
            &mut lines,
            "DISTRIBUTED ROUTE DEMO proof=software std+browser".into(),
        )?;
        push(
            &mut lines,
            format!(
                "FORM source={} checked={} semantic-host-facts=none semantic-carrier-facts=none",
                one.source_document_id.as_str(),
                one.checked_form_id.as_str()
            ),
        )?;
        push(
            &mut lines,
            format!(
                "  PEERS presenter=patchbay-native planned-hosts={}",
                one.fragments
                    .iter()
                    .map(|fragment| fragment.host_id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )?;
        render_plan(&mut lines, "PLAN-A replan-required", &one, one_connection)?;

        let mut one_machine = RouteMachine::new(one_connection)?;
        let lost = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from(USB_ROUTE),
            availability: LinkAvailability::Unavailable,
            clue_id: ClueId::from("route-demo/plan-a/usb-unavailable"),
        };
        let update = one_machine.observe(&lost)?;
        if !matches!(
            update.disposition,
            RouteDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ) {
            return Err(RouteDemoError::InvalidClue);
        }
        let connection_id = one_connection.connection_id.clone();
        let events = [
            ControlLoopEvent::LinkBecameUnavailable {
                plan_id: one.plan_id.clone(),
                connection_id: connection_id.clone(),
                binding_id: lost.binding_id.clone(),
                observation_clue_id: lost.clue_id.clone(),
            },
            ControlLoopEvent::PlayBecameUnsatisfied {
                plan_id: one.plan_id.clone(),
                reason: PlayUnsatisfiedReason::NoAdmittedRouteReady,
                clue_id: ClueId::from("route-demo/plan-a/unsatisfied"),
            },
            ControlLoopEvent::PlanningRequested {
                prior_plan_id: one.plan_id.clone(),
                requester_host_id: source_host_id,
                requester_boot_id: source_boot_id,
                authority: PlanningRequestAuthority::HostLocal,
                request_clue_id: ClueId::from("route-demo/plan-a/replan-request"),
            },
            ControlLoopEvent::PlanningSucceeded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                request_clue_id: ClueId::from("route-demo/plan-a/replan-request"),
                clue_id: ClueId::from("route-demo/plan-c/planned"),
            },
            ControlLoopEvent::PlanSuperseded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                clue_id: ClueId::from("route-demo/plan-c/installed"),
            },
        ];
        for event in &events {
            event.validate().map_err(|_| RouteDemoError::InvalidClue)?;
            if matches!(event, ControlLoopEvent::LinkBecameUnavailable { .. }) {
                event
                    .validate_route_event(&one.plan_id, one_connection)
                    .map_err(|_| RouteDemoError::InvalidClue)?;
            }
            push(&mut lines, format!("  CLUE {event:?}"))?;
        }
        push(
            &mut lines,
            format!(
                "  OUTCOME replan=true prior-plan={} replacement-plan={} play=awaiting-realization",
                one.plan_id.as_str(),
                replacement.plan_id.as_str()
            ),
        )?;

        render_plan(
            &mut lines,
            "PLAN-B predeclared-fallback",
            &fallback,
            fallback_connection,
        )?;
        let mut fallback_machine = RouteMachine::new(fallback_connection)?;
        let ready = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from(conduit_signal::DISTRIBUTED_LINK_BINDING_ID),
            availability: LinkAvailability::Ready,
            clue_id: ClueId::from("route-demo/plan-b/websocket-ready"),
        };
        fallback_machine.observe(&ready)?;
        let fallback_lost = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from(USB_ROUTE),
            availability: LinkAvailability::Unavailable,
            clue_id: ClueId::from("route-demo/plan-b/usb-unavailable"),
        };
        let changed = fallback_machine.observe(&fallback_lost)?;
        let RouteDisposition::Selected { link, .. } = changed.disposition else {
            return Err(RouteDemoError::InvalidClue);
        };
        let selected_binding_id = link.binding_id.clone();
        let selection = ControlLoopEvent::RouteSelectionChanged {
            plan_id: fallback.plan_id.clone(),
            connection_id: fallback_connection.connection_id.clone(),
            previous_binding_id: changed.previous_selection.cloned(),
            selected_binding_id: link.binding_id.clone(),
            observation_clue_id: fallback_lost.clue_id.clone(),
        };
        selection
            .validate_route_event(&fallback.plan_id, fallback_connection)
            .map_err(|_| RouteDemoError::InvalidClue)?;
        push(&mut lines, format!("  OBSERVATION {fallback_lost:?}"))?;
        push(&mut lines, format!("  CLUE {selection:?}"))?;
        push(
            &mut lines,
            format!(
                "  OUTCOME replan=false same-plan={} selected={} play=continues",
                fallback.plan_id.as_str(),
                selected_binding_id.as_str()
            ),
        )?;

        let invented = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from("ambient/unplanned-wifi"),
            availability: LinkAvailability::Ready,
            clue_id: ClueId::from("route-demo/ambient-observation"),
        };
        let refusal = fallback_machine.observe(&invented);
        if refusal != Err(RouteError::UnsealedObservation) {
            return Err(RouteDemoError::InvalidClue);
        }
        push(
            &mut lines,
            "  REFUSED route=ambient/unplanned-wifi reason=UnsealedObservation plan-unchanged=true"
                .into(),
        )?;
        let [link_event, unsatisfied_event, request_event, success_event, installed_event] =
            &events;
        let ControlLoopEvent::LinkBecameUnavailable {
            observation_clue_id: unavailable_clue_id,
            ..
        } = link_event
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let ControlLoopEvent::PlayBecameUnsatisfied {
            clue_id: unsatisfied_clue_id,
            ..
        } = unsatisfied_event
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let ControlLoopEvent::PlanningRequested {
            request_clue_id: planning_request_clue_id,
            ..
        } = request_event
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let ControlLoopEvent::PlanningSucceeded {
            clue_id: planning_success_clue_id,
            ..
        } = success_event
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let ControlLoopEvent::PlanSuperseded {
            clue_id: installed_clue_id,
            ..
        } = installed_event
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let ControlLoopEvent::RouteSelectionChanged {
            observation_clue_id: selection_clue_id,
            ..
        } = &selection
        else {
            return Err(RouteDemoError::InvalidClue);
        };
        let presentation = DistributedRoutePresentation {
            source_document_id: one.source_document_id.clone(),
            checked_form_id: one.checked_form_id.clone(),
            new_plan: NewPlanRecoveryPresentation {
                prior: plan_presentation(&one, one_connection),
                replacement_plan_id: replacement.plan_id.clone(),
                unavailable_binding_id: lost.binding_id,
                unavailable_clue_id: unavailable_clue_id.clone(),
                unsatisfied_clue_id: unsatisfied_clue_id.clone(),
                planning_request_clue_id: planning_request_clue_id.clone(),
                planning_success_clue_id: planning_success_clue_id.clone(),
                installed_clue_id: installed_clue_id.clone(),
            },
            same_plan: SamePlanFallbackPresentation {
                plan: plan_presentation(&fallback, fallback_connection),
                unavailable_binding_id: fallback_lost.binding_id,
                unavailable_clue_id: selection_clue_id.clone(),
                selected_binding_id,
                selection_clue_id: selection_clue_id.clone(),
            },
            refused: RefusedRoutePresentation {
                binding_id: invented.binding_id,
                observation_clue_id: invented.clue_id,
            },
        };
        let mut control_events = events.to_vec();
        control_events.push(selection);
        Ok(Self {
            lines,
            presentation,
            control_events,
        })
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn presentation(&self) -> &DistributedRoutePresentation {
        &self.presentation
    }

    pub fn control_events(&self) -> &[ControlLoopEvent] {
        &self.control_events
    }

    pub fn visual_lines(&self) -> Vec<String> {
        self.presentation.visual_lines()
    }

    pub fn linear_lines(&self) -> Vec<String> {
        self.presentation.linear_lines()
    }
}

fn plan_presentation(
    plan: &conduit_core::Plan,
    connection: &conduit_core::PlannedConnection,
) -> RoutePlanPresentation {
    RoutePlanPresentation {
        plan_id: plan.plan_id.clone(),
        connection_id: connection.connection_id.clone(),
        candidates: connection
            .route_candidates
            .iter()
            .enumerate()
            .map(|(order, candidate)| RouteCandidatePresentation {
                order,
                binding_id: candidate.binding_id.clone(),
                base: candidate.base,
                base_instance_id: candidate.base_instance_id.clone(),
            })
            .collect(),
    }
}

fn planned(
    candidate_ids: &[&str],
    source_host_id: &HostId,
    source_boot_id: &BootId,
) -> Result<conduit_core::Plan, RouteDemoError> {
    let source = native_advertisement(source_host_id.clone(), source_boot_id.clone());
    let sink = distributed_browser_sink_advertisement();
    let syntax = conduit_form::parse_syntax_document(SOURCE);
    let checked =
        conduit_form::check_syntax_document(&syntax, &conduit_signal::signal_startup_catalog())
            .map_err(|_| RouteDemoError::InvalidSemanticForm)?;
    let form =
        conduit_form::expand_canonical_form(&checked, "signal-demo", &signal_profile_catalog())
            .map_err(|_| RouteDemoError::InvalidSemanticForm)?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: CapabilityId::from("dom-show-1"),
                },
            ),
        ]),
    };
    let mut websocket = distributed_websocket_link_binding();
    websocket.source.host_id = source.host_id.clone();
    websocket.source.boot_id = source.boot_id.clone();
    let mut usb = websocket.clone();
    usb.binding_id = LinkBindingId::from(USB_ROUTE);
    usb.base = ConnectionBase::UsbCdc;
    usb.base_instance_id = "route-demo/usb-base".into();
    usb.source.endpoint_id = LinkEndpointId::from("route-demo/usb-source");
    usb.sink.endpoint_id = LinkEndpointId::from("route-demo/usb-sink");
    let links = [usb, websocket];
    let route_candidates = BTreeMap::from([(
        (
            GearId::from("signal-demo/pulse"),
            GearId::from("signal-demo/show"),
        ),
        candidate_ids
            .iter()
            .copied()
            .map(LinkBindingId::from)
            .collect(),
    )]);
    plan_expanded_canonical_with_options(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionBase::WebSocket, ConnectionBase::UsbCdc],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            route_candidates: &route_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            link_bindings: &links,
        },
    )
    .map_err(|error| RouteDemoError::Planning(error.to_string()))
}

fn native_advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    StdHost::new_with_composition(
        StdHostConfig {
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_signal(),
    )
    .advertisement()
    .clone()
}

fn remote_connection(
    plan: &conduit_core::Plan,
) -> Result<&conduit_core::PlannedConnection, RouteDemoError> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| !connection.route_candidates.is_empty())
        .ok_or(RouteDemoError::MissingConnection)
}

fn render_plan(
    lines: &mut Vec<String>,
    label: &str,
    plan: &conduit_core::Plan,
    connection: &conduit_core::PlannedConnection,
) -> Result<(), RouteDemoError> {
    push(lines, format!("{label} id={}", plan.plan_id.as_str()))?;
    push(
        lines,
        format!(
            "  CORD connection={} exact-plan={}",
            connection.connection_id.as_str(),
            plan.plan_id.as_str()
        ),
    )?;
    for (index, candidate) in connection.route_candidates.iter().enumerate() {
        push(
            lines,
            format!(
                "    CANDIDATE order={} binding={} base={:?} base-instance={} source-endpoint={} sink-endpoint={} availability=runtime-observation",
                index,
                candidate.binding_id.as_str(),
                candidate.base,
                candidate.base_instance_id.as_str(),
                candidate.source.endpoint_id.as_str(),
                candidate.sink.endpoint_id.as_str()
            ),
        )?;
    }
    Ok(())
}

fn push(lines: &mut Vec<String>, line: String) -> Result<(), RouteDemoError> {
    if lines.len() == 64 {
        return Err(RouteDemoError::InspectionTooLarge);
    }
    lines.push(line);
    Ok(())
}

#[cfg(test)]
#[path = "route_demo_tests.rs"]
mod tests;
