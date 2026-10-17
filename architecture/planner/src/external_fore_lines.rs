//! Exact carrier selection for an already planned external Fore.
//!
//! This seals immutable Line facts only. Current availability is checked when
//! selecting and must be checked again by the carrier before use.

use crate::{contract::LineMechanismPolicy, validate_line_offers, PlannerError};
use alloc::collections::BTreeSet;
use conduit_core::{
    seal_plan_with_activation_entries, verify_plan, BaseImplementationId, BootId, ConnectionTrack,
    HostId, LineAvailability, LineId, LineOffer, Plan, PlotIdentity, PortDirection, PortId,
};

/// A host-owned Fore and its explicitly chosen remote peer and offered Line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalForeLineChoice {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub direction: PortDirection,
    pub front_port_id: PortId,
    pub track: ConnectionTrack,
    pub peer_host_id: HostId,
    pub peer_boot_id: BootId,
    pub line_id: LineId,
}

/// Return a freshly sealed Plan whose chosen external Fores carry exact Lines.
/// No offer, policy, or mutable availability fact is inserted into authored Plot.
pub fn bind_external_fore_lines(
    plan: &Plan,
    choices: &[ExternalForeLineChoice],
    offers: &[LineOffer],
    allowed_bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    if !verify_plan(plan) {
        return Err(PlannerError::InvalidPlotIdentity(
            "external Fore Line selection requires one verified Plan".into(),
        ));
    }
    validate_line_offers(offers)?;
    let policy = LineMechanismPolicy::new(allowed_bases);
    let mut selected = plan.clone();
    let mut seen = BTreeSet::new();
    for choice in choices {
        let key = (
            &choice.host_id,
            &choice.boot_id,
            choice.direction as u8,
            &choice.front_port_id,
            choice.track,
        );
        if !seen.insert(key) {
            return Err(PlannerError::InvalidLineOffer(
                "external Fore Line choice is duplicated".into(),
            ));
        }
        let fore = selected
            .fragments
            .iter_mut()
            .filter(|fragment| {
                fragment.host_id == choice.host_id && fragment.boot_id == choice.boot_id
            })
            .flat_map(|fragment| &mut fragment.fore_ports)
            .find(|fore| {
                fore.direction == choice.direction
                    && fore.front_port_id == choice.front_port_id
                    && fore.track == choice.track
            })
            .ok_or_else(|| {
                PlannerError::InvalidPlotIdentity(format!(
                    "external Fore '{}' is absent from the selected Host Boot",
                    choice.front_port_id.as_str()
                ))
            })?;
        if fore.selected_line.is_some() {
            return Err(PlannerError::InvalidLineOffer(format!(
                "external Fore '{}' already has a selected Line",
                choice.front_port_id.as_str()
            )));
        }
        let mut matching = offers
            .iter()
            .filter(|offer| offer.line_id == choice.line_id);
        let offer = matching.next().ok_or_else(|| {
            PlannerError::LineOfferMissing(format!(
                "requested external Fore Line '{}' is not offered",
                choice.line_id.as_str()
            ))
        })?;
        if matching.next().is_some() {
            return Err(PlannerError::LineOfferAmbiguous(format!(
                "external Fore Line '{}' has more than one offer",
                choice.line_id.as_str()
            )));
        }
        let (local, peer) = match choice.direction {
            PortDirection::Input => (&offer.binding.sink, &offer.binding.source),
            PortDirection::Output => (&offer.binding.source, &offer.binding.sink),
        };
        if local.host_id != choice.host_id
            || local.boot_id != choice.boot_id
            || peer.host_id != choice.peer_host_id
            || peer.boot_id != choice.peer_boot_id
            || !policy.permits_remote(&offer.binding.base)
        {
            return Err(PlannerError::LineOfferMissing(format!(
                "Line '{}' does not match external Fore direction, Host Boot, peer, or allowed Base",
                choice.line_id.as_str()
            )));
        }
        if offer.availability.availability != LineAvailability::Ready
            || offer.binding.limits.maximum_in_flight_items < fore.item_capacity
            || offer.binding.limits.maximum_payload_bytes < fore.byte_capacity
            || offer.binding.limits.maximum_buffered_bytes < fore.byte_capacity
        {
            return Err(PlannerError::LineOfferUnavailable(format!(
                "Line '{}' is unavailable or below the external Fore contract",
                choice.line_id.as_str()
            )));
        }
        fore.selected_line = Some(offer.admitted_line());
    }
    if choices.is_empty() {
        return Ok(selected);
    }
    let resealed = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.completion_policy,
        plan.realization_backs.clone(),
        plan.activations.clone(),
        selected.fragments,
    );
    if !verify_plan(&resealed) {
        return Err(PlannerError::InvalidLineOffer(
            "selected external Fore Lines do not form a valid Plan".into(),
        ));
    }
    Ok(resealed)
}
