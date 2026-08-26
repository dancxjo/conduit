//! Sign-backed projection of distributed route selection and replanning.

use conduit_core::{
    BootId, CapabilityId, ConnectionBase, ControlLoopEvent, GearId, HostAdvertisement, HostId,
    LineAvailability, LineAvailabilitySign, LineId, LinkBindingId, LinkEndpointId, OfferGeneration,
    PlanningRequestAuthority, PlayUnsatisfiedReason, SignId,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_signal::{signal_profile_catalog, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{
    distributed_browser_sink_advertisement, distributed_websocket_line_offer,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use conduit_wire::{LineDisposition, LineError, LineMachine};
use std::collections::BTreeMap;

use crate::route_presentation::{
    DistributedRoutePresentation, NewPlanRecoveryPresentation, RefusedRoutePresentation,
    RouteCandidatePresentation, RoutePlanPresentation, SamePlanFallbackPresentation,
};

const SOURCE: &str = include_str!("../../../examples/signal-demo.conduit");
const USB_LINE: &str = "s4/line/distributed-signal-usb";
const USB_BINDING: &str = "s4/distributed-signal-usb-link";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDemoError {
    InvalidSemanticForm,
    Planning(String),
    MissingConnection,
    Line(LineError),
    InvalidSign,
    InspectionTooLarge,
}

impl From<LineError> for RouteDemoError {
    fn from(value: LineError) -> Self {
        Self::Line(value)
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
        let one = planned(&[USB_LINE], &source_host_id, &source_boot_id)?;
        let fallback = planned(
            &[USB_LINE, conduit_signal_conformance::DISTRIBUTED_LINE_ID],
            &source_host_id,
            &source_boot_id,
        )?;
        let replacement = planned(
            &[conduit_signal_conformance::DISTRIBUTED_LINE_ID],
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
                "FORM source={} checked={} semantic-host-facts=none semantic-line-facts=none",
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

        let mut one_machine = LineMachine::new(one_connection)?;
        let lost = LineAvailabilitySign {
            line_id: LineId::from(USB_LINE),
            binding_id: LinkBindingId::from(USB_BINDING),
            availability: LineAvailability::Unavailable,
            sign_id: SignId::from("route-demo/plan-a/usb-unavailable"),
        };
        let update = one_machine.observe(&lost)?;
        if !matches!(
            update.disposition,
            LineDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ) {
            return Err(RouteDemoError::InvalidSign);
        }
        let connection_id = one_connection.connection_id.clone();
        let events = [
            ControlLoopEvent::LineBecameUnavailable {
                plan_id: one.plan_id.clone(),
                connection_id: connection_id.clone(),
                line_id: lost.line_id.clone(),
                binding_id: lost.binding_id.clone(),
                observation_sign_id: lost.sign_id.clone(),
            },
            ControlLoopEvent::PlayBecameUnsatisfied {
                plan_id: one.plan_id.clone(),
                reason: PlayUnsatisfiedReason::NoAdmittedRouteReady,
                sign_id: SignId::from("route-demo/plan-a/unsatisfied"),
            },
            ControlLoopEvent::PlanningRequested {
                prior_plan_id: one.plan_id.clone(),
                requester_host_id: source_host_id,
                requester_boot_id: source_boot_id,
                authority: PlanningRequestAuthority::HostLocal,
                request_sign_id: SignId::from("route-demo/plan-a/replan-request"),
            },
            ControlLoopEvent::PlanningSucceeded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                request_sign_id: SignId::from("route-demo/plan-a/replan-request"),
                sign_id: SignId::from("route-demo/plan-c/planned"),
            },
            ControlLoopEvent::PlanSuperseded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                sign_id: SignId::from("route-demo/plan-c/installed"),
            },
        ];
        for event in &events {
            event.validate().map_err(|_| RouteDemoError::InvalidSign)?;
            if matches!(event, ControlLoopEvent::LineBecameUnavailable { .. }) {
                event
                    .validate_route_event(&one.plan_id, one_connection)
                    .map_err(|_| RouteDemoError::InvalidSign)?;
            }
            push(&mut lines, format!("  SIGN {event:?}"))?;
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
        let mut fallback_machine = LineMachine::new(fallback_connection)?;
        let ready = LineAvailabilitySign {
            line_id: LineId::from(conduit_signal_conformance::DISTRIBUTED_LINE_ID),
            binding_id: LinkBindingId::from(
                conduit_signal_conformance::DISTRIBUTED_LINK_BINDING_ID,
            ),
            availability: LineAvailability::Ready,
            sign_id: SignId::from("route-demo/plan-b/websocket-ready"),
        };
        fallback_machine.observe(&ready)?;
        let fallback_lost = LineAvailabilitySign {
            line_id: LineId::from(USB_LINE),
            binding_id: LinkBindingId::from(USB_BINDING),
            availability: LineAvailability::Unavailable,
            sign_id: SignId::from("route-demo/plan-b/usb-unavailable"),
        };
        let changed = fallback_machine.observe(&fallback_lost)?;
        let LineDisposition::Selected { line, .. } = changed.disposition else {
            return Err(RouteDemoError::InvalidSign);
        };
        let selected_binding_id = line.binding.binding_id.clone();
        let selection = ControlLoopEvent::LineSelectionChanged {
            plan_id: fallback.plan_id.clone(),
            connection_id: fallback_connection.connection_id.clone(),
            previous_line_id: changed.previous_selection.cloned(),
            selected_line_id: line.line_id.clone(),
            selected_binding_id: line.binding.binding_id.clone(),
            observation_sign_id: fallback_lost.sign_id.clone(),
        };
        selection
            .validate_route_event(&fallback.plan_id, fallback_connection)
            .map_err(|_| RouteDemoError::InvalidSign)?;
        push(&mut lines, format!("  OBSERVATION {fallback_lost:?}"))?;
        push(&mut lines, format!("  SIGN {selection:?}"))?;
        push(
            &mut lines,
            format!(
                "  OUTCOME replan=false same-plan={} selected={} play=continues",
                fallback.plan_id.as_str(),
                selected_binding_id.as_str()
            ),
        )?;

        let invented = LineAvailabilitySign {
            line_id: LineId::from("ambient/unplanned-wifi"),
            binding_id: LinkBindingId::from("ambient/unplanned-wifi"),
            availability: LineAvailability::Ready,
            sign_id: SignId::from("route-demo/ambient-observation"),
        };
        let refusal = fallback_machine.observe(&invented);
        if refusal != Err(LineError::UnsealedObservation) {
            return Err(RouteDemoError::InvalidSign);
        }
        push(
            &mut lines,
            "  REFUSED route=ambient/unplanned-wifi reason=UnsealedObservation plan-unchanged=true"
                .into(),
        )?;
        let [link_event, unsatisfied_event, request_event, success_event, installed_event] =
            &events;
        let ControlLoopEvent::LineBecameUnavailable {
            observation_sign_id: unavailable_sign_id,
            ..
        } = link_event
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let ControlLoopEvent::PlayBecameUnsatisfied {
            sign_id: unsatisfied_sign_id,
            ..
        } = unsatisfied_event
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let ControlLoopEvent::PlanningRequested {
            request_sign_id: planning_request_sign_id,
            ..
        } = request_event
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let ControlLoopEvent::PlanningSucceeded {
            sign_id: planning_success_sign_id,
            ..
        } = success_event
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let ControlLoopEvent::PlanSuperseded {
            sign_id: installed_sign_id,
            ..
        } = installed_event
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let ControlLoopEvent::LineSelectionChanged {
            observation_sign_id: selection_sign_id,
            ..
        } = &selection
        else {
            return Err(RouteDemoError::InvalidSign);
        };
        let presentation = DistributedRoutePresentation {
            source_document_id: one.source_document_id.clone(),
            checked_form_id: one.checked_form_id.clone(),
            new_plan: NewPlanRecoveryPresentation {
                prior: plan_presentation(&one, one_connection),
                replacement_plan_id: replacement.plan_id.clone(),
                unavailable_binding_id: lost.binding_id,
                unavailable_sign_id: unavailable_sign_id.clone(),
                unsatisfied_sign_id: unsatisfied_sign_id.clone(),
                planning_request_sign_id: planning_request_sign_id.clone(),
                planning_success_sign_id: planning_success_sign_id.clone(),
                installed_sign_id: installed_sign_id.clone(),
            },
            same_plan: SamePlanFallbackPresentation {
                plan: plan_presentation(&fallback, fallback_connection),
                unavailable_binding_id: fallback_lost.binding_id,
                unavailable_sign_id: selection_sign_id.clone(),
                selected_binding_id,
                selection_sign_id: selection_sign_id.clone(),
            },
            refused: RefusedRoutePresentation {
                binding_id: invented.binding_id,
                observation_sign_id: invented.sign_id,
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
            .admitted_lines
            .iter()
            .enumerate()
            .map(|(order, candidate)| RouteCandidatePresentation {
                order,
                binding_id: candidate.binding.binding_id.clone(),
                base: candidate.binding.base,
                base_instance_id: candidate.binding.base_instance_id.clone(),
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
    let mut websocket = distributed_websocket_line_offer();
    websocket.binding.source.host_id = source.host_id.clone();
    websocket.binding.source.boot_id = source.boot_id.clone();
    let mut usb = websocket.clone();
    usb.line_id = LineId::from(USB_LINE);
    usb.binding.binding_id = LinkBindingId::from(USB_BINDING);
    usb.binding.base = ConnectionBase::UsbCdc;
    usb.binding.base_instance_id = "route-demo/usb-base".into();
    usb.binding.source.endpoint_id = LinkEndpointId::from("route-demo/usb-source");
    usb.binding.sink.endpoint_id = LinkEndpointId::from("route-demo/usb-sink");
    usb.availability.line_id = usb.line_id.clone();
    usb.availability.binding_id = usb.binding.binding_id.clone();
    let lines = [usb, websocket];
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("signal-demo/pulse"),
            GearId::from("signal-demo/show"),
        ),
        candidate_ids.iter().copied().map(LineId::from).collect(),
    )]);
    plan_expanded_canonical_with_options(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionBase::WebSocket, ConnectionBase::UsbCdc],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
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
        .find(|connection| !connection.admitted_lines.is_empty())
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
    for (index, candidate) in connection.admitted_lines.iter().enumerate() {
        push(
            lines,
            format!(
                "    CANDIDATE order={} binding={} base={:?} base-instance={} source-endpoint={} sink-endpoint={} availability=runtime-observation",
                index,
                candidate.binding.binding_id.as_str(),
                candidate.binding.base,
                candidate.binding.base_instance_id.as_str(),
                candidate.binding.source.endpoint_id.as_str(),
                candidate.binding.sink.endpoint_id.as_str()
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
