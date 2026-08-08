//! Evidence-backed projection of distributed route selection and replanning.

use conduit_core::{
    BootId, CapabilityId, ConnectionProvider, ControlLoopEvent, DeploymentUnsatisfiedReason,
    EvidenceId, HostAdvertisement, HostId, LinkAvailability, LinkBindingId, LinkEndpointId,
    OfferGeneration, OperationId, PlanningRequestAuthority,
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

const SOURCE: &str = include_str!("../../../examples/signal-demo.conduit");
const USB_ROUTE: &str = "s4/distributed-signal-usb-link";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDemoError {
    InvalidSemanticForm,
    Planning(String),
    MissingConnection,
    Route(RouteError),
    InvalidEvidence,
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
            evidence_id: EvidenceId::from("route-demo/plan-a/usb-unavailable"),
        };
        let update = one_machine.observe(&lost)?;
        if !matches!(
            update.disposition,
            RouteDisposition::Unsatisfied {
                replan_may_be_requested: true
            }
        ) {
            return Err(RouteDemoError::InvalidEvidence);
        }
        let connection_id = one_connection.connection_id.clone();
        let events = [
            ControlLoopEvent::LinkBecameUnavailable {
                plan_id: one.plan_id.clone(),
                connection_id: connection_id.clone(),
                binding_id: lost.binding_id.clone(),
                observation_evidence_id: lost.evidence_id.clone(),
            },
            ControlLoopEvent::DeploymentBecameUnsatisfied {
                plan_id: one.plan_id.clone(),
                reason: DeploymentUnsatisfiedReason::NoAdmittedRouteReady,
                evidence_id: EvidenceId::from("route-demo/plan-a/unsatisfied"),
            },
            ControlLoopEvent::PlanningRequested {
                prior_plan_id: one.plan_id.clone(),
                requester_host_id: source_host_id,
                requester_boot_id: source_boot_id,
                authority: PlanningRequestAuthority::HostLocal,
                request_evidence_id: EvidenceId::from("route-demo/plan-a/replan-request"),
            },
            ControlLoopEvent::PlanningSucceeded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                request_evidence_id: EvidenceId::from("route-demo/plan-a/replan-request"),
                evidence_id: EvidenceId::from("route-demo/plan-c/planned"),
            },
            ControlLoopEvent::PlanSuperseded {
                prior_plan_id: one.plan_id.clone(),
                replacement_plan_id: replacement.plan_id.clone(),
                evidence_id: EvidenceId::from("route-demo/plan-c/installed"),
            },
        ];
        for event in &events {
            event
                .validate()
                .map_err(|_| RouteDemoError::InvalidEvidence)?;
            if matches!(event, ControlLoopEvent::LinkBecameUnavailable { .. }) {
                event
                    .validate_route_event(&one.plan_id, one_connection)
                    .map_err(|_| RouteDemoError::InvalidEvidence)?;
            }
            push(&mut lines, format!("  EVIDENCE {event:?}"))?;
        }
        push(
            &mut lines,
            format!(
                "  OUTCOME replan=true prior-plan={} replacement-plan={} play=awaiting-deployment",
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
            evidence_id: EvidenceId::from("route-demo/plan-b/websocket-ready"),
        };
        fallback_machine.observe(&ready)?;
        let fallback_lost = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from(USB_ROUTE),
            availability: LinkAvailability::Unavailable,
            evidence_id: EvidenceId::from("route-demo/plan-b/usb-unavailable"),
        };
        let changed = fallback_machine.observe(&fallback_lost)?;
        let RouteDisposition::Selected { link, .. } = changed.disposition else {
            return Err(RouteDemoError::InvalidEvidence);
        };
        let selection = ControlLoopEvent::RouteSelectionChanged {
            plan_id: fallback.plan_id.clone(),
            connection_id: fallback_connection.connection_id.clone(),
            previous_binding_id: changed.previous_selection.cloned(),
            selected_binding_id: link.binding_id.clone(),
            observation_evidence_id: fallback_lost.evidence_id.clone(),
        };
        selection
            .validate_route_event(&fallback.plan_id, fallback_connection)
            .map_err(|_| RouteDemoError::InvalidEvidence)?;
        push(&mut lines, format!("  OBSERVATION {fallback_lost:?}"))?;
        push(&mut lines, format!("  EVIDENCE {selection:?}"))?;
        push(
            &mut lines,
            format!(
                "  OUTCOME replan=false same-plan={} selected={} play=continues",
                fallback.plan_id.as_str(),
                link.binding_id.as_str()
            ),
        )?;

        let invented = conduit_core::LinkObservation {
            binding_id: LinkBindingId::from("ambient/unplanned-wifi"),
            availability: LinkAvailability::Ready,
            evidence_id: EvidenceId::from("route-demo/ambient-observation"),
        };
        let refusal = fallback_machine.observe(&invented);
        if refusal != Err(RouteError::UnsealedObservation) {
            return Err(RouteDemoError::InvalidEvidence);
        }
        push(
            &mut lines,
            "  REFUSED route=ambient/unplanned-wifi reason=UnsealedObservation plan-unchanged=true"
                .into(),
        )?;
        Ok(Self { lines })
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
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
        by_operation: BTreeMap::from([
            (
                OperationId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                OperationId::from("signal-demo/show"),
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
    usb.provider = ConnectionProvider::UsbCdc;
    usb.provider_instance_id = "route-demo/usb-provider".into();
    usb.source.endpoint_id = LinkEndpointId::from("route-demo/usb-source");
    usb.sink.endpoint_id = LinkEndpointId::from("route-demo/usb-sink");
    let links = [usb, websocket];
    let route_candidates = BTreeMap::from([(
        (
            OperationId::from("signal-demo/pulse"),
            OperationId::from("signal-demo/show"),
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
        &[ConnectionProvider::WebSocket, ConnectionProvider::UsbCdc],
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
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
                "    CANDIDATE order={} binding={} provider={:?} availability=runtime-observation",
                index,
                candidate.binding_id.as_str(),
                candidate.provider
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
mod tests {
    use super::*;

    #[test]
    fn document_distinguishes_replan_from_same_plan_fallback() {
        let demo = DistributedRouteDemo::build().expect("route demo");
        let text = demo.lines().join("\n");
        assert!(text.contains("semantic-host-facts=none semantic-carrier-facts=none"));
        assert!(text.contains("PLAN-A replan-required"));
        assert!(text.contains("OUTCOME replan=true prior-plan="));
        assert!(text.contains("PLAN-B predeclared-fallback"));
        assert!(text.contains("OUTCOME replan=false same-plan="));
        assert!(text.contains("REFUSED route=ambient/unplanned-wifi"));
        assert!(text.contains("patchbay-native/std-realization"));
        assert!(text.contains(conduit_signal::DISTRIBUTED_BROWSER_HOST_ID));
    }

    #[test]
    fn candidate_order_changes_exact_plan_identity() {
        let host = HostId::from("patchbay-native/std-realization");
        let boot = BootId::from("patchbay-native/std-boot-1");
        let usb_first = planned(
            &[USB_ROUTE, conduit_signal::DISTRIBUTED_LINK_BINDING_ID],
            &host,
            &boot,
        )
        .unwrap();
        let websocket_first = planned(
            &[conduit_signal::DISTRIBUTED_LINK_BINDING_ID, USB_ROUTE],
            &host,
            &boot,
        )
        .unwrap();
        assert_ne!(usb_first.plan_id, websocket_first.plan_id);
    }
}
