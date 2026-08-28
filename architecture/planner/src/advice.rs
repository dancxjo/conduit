//! Finite planner advice that remains subordinate to ordinary planning.

use crate::prelude::*;
use crate::{PlacementChoice, PlacementChoices, PlannerError};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    BootId, CapabilityId, CheckedFormId, GearId, HostAdvertisement, HostId, LineAvailability,
    LineId, LineOffer, OfferGeneration,
};
use conduit_form::CheckedForm;

pub const MAXIMUM_ADVICE_PLACEMENTS: usize = 64;
pub const MAXIMUM_ADVICE_LINES: usize = 64;
pub const MAXIMUM_ADVICE_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedPlacement {
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedLine {
    pub source_gear_id: GearId,
    pub sink_gear_id: GearId,
    pub line_id: LineId,
}

/// Typed model output. It deliberately has no PlanId, fragment, reservation,
/// authority, or active-Play field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningAdvice {
    pub proposal_id: String,
    pub request_identity: String,
    pub run_identity: String,
    pub checked_form_id: CheckedFormId,
    pub placements: Vec<SuggestedPlacement>,
    pub lines: Vec<SuggestedLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningAdviceEvidence {
    pub proposal_id: String,
    pub request_identity: String,
    pub run_identity: String,
    pub checked_form_id: CheckedFormId,
    pub proposed_placements: u16,
    pub used_placements: u16,
    pub proposed_lines: u16,
    pub used_lines: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisedPlanningInputs {
    pub placements: PlacementChoices,
    pub line_candidates: BTreeMap<(GearId, GearId), Vec<LineId>>,
    /// Provenance is planning evidence, not an input to Plan sealing.
    pub evidence: PlanningAdviceEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningAdviceRefusal {
    EmptyIdentity,
    IdentityTooLong,
    PlacementCapacityExceeded,
    LineCapacityExceeded,
    WrongForm,
    UnknownGear,
    DuplicateGear,
    UnknownHost,
    StaleBoot,
    StaleOfferGeneration,
    UnknownCapability,
    UnknownConnection,
    DuplicateConnection,
    UnknownLine,
    LineUnavailable,
    LineEndpointMismatch,
    OrdinaryPlanningUnavailable,
}

/// Revalidates advice against current exact truth and returns only ordinary
/// planner inputs. The caller must still invoke `plan_with_options`, which
/// remains authoritative for semantic compatibility, resources, authority,
/// queue bounds, Lines, and immutable Plan identity.
pub fn seed_planning_from_advice(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    line_offers: &[LineOffer],
    advice: &PlanningAdvice,
) -> Result<AdvisedPlanningInputs, PlanningAdviceRefusal> {
    validate_identity(&advice.proposal_id)?;
    validate_identity(&advice.request_identity)?;
    validate_identity(&advice.run_identity)?;
    if advice.placements.len() > MAXIMUM_ADVICE_PLACEMENTS {
        return Err(PlanningAdviceRefusal::PlacementCapacityExceeded);
    }
    if advice.lines.len() > MAXIMUM_ADVICE_LINES {
        return Err(PlanningAdviceRefusal::LineCapacityExceeded);
    }
    if advice.checked_form_id != form.checked_form_id {
        return Err(PlanningAdviceRefusal::WrongForm);
    }

    let mut placements = PlacementChoices {
        by_gear: BTreeMap::new(),
    };
    let mut proposed_gears = BTreeSet::new();
    for suggestion in &advice.placements {
        if !form
            .gears
            .iter()
            .any(|gear| gear.gear_id == suggestion.gear_id)
        {
            return Err(PlanningAdviceRefusal::UnknownGear);
        }
        if !proposed_gears.insert(suggestion.gear_id.clone()) {
            return Err(PlanningAdviceRefusal::DuplicateGear);
        }
        let current = current_host(hosts, suggestion)?;
        if !current
            .capabilities
            .iter()
            .any(|offer| offer.capability_id == suggestion.capability_id)
        {
            return Err(PlanningAdviceRefusal::UnknownCapability);
        }
        placements.by_gear.insert(
            suggestion.gear_id.clone(),
            PlacementChoice {
                host_id: suggestion.host_id.clone(),
                capability_id: suggestion.capability_id.clone(),
            },
        );
    }
    for gear in &form.gears {
        if placements.by_gear.contains_key(&gear.gear_id) {
            continue;
        }
        let default = crate::functional_compatibility::default_placements_unvalidated(
            core::slice::from_ref(gear),
            hosts,
        )
        .map_err(|_: PlannerError| PlanningAdviceRefusal::OrdinaryPlanningUnavailable)?;
        placements.by_gear.extend(default.by_gear);
    }

    let mut line_candidates = BTreeMap::new();
    for suggestion in &advice.lines {
        let connection = form
            .connections
            .iter()
            .find(|connection| {
                connection.source_gear_id == suggestion.source_gear_id
                    && connection.sink_gear_id == suggestion.sink_gear_id
            })
            .ok_or(PlanningAdviceRefusal::UnknownConnection)?;
        let key = (
            connection.source_gear_id.clone(),
            connection.sink_gear_id.clone(),
        );
        if line_candidates.contains_key(&key) {
            return Err(PlanningAdviceRefusal::DuplicateConnection);
        }
        let offer = line_offers
            .iter()
            .find(|offer| offer.line_id == suggestion.line_id)
            .ok_or(PlanningAdviceRefusal::UnknownLine)?;
        if offer.availability.availability != LineAvailability::Ready {
            return Err(PlanningAdviceRefusal::LineUnavailable);
        }
        let source = selected_host(&placements, hosts, &connection.source_gear_id)?;
        let sink = selected_host(&placements, hosts, &connection.sink_gear_id)?;
        if offer.binding.source.host_id != source.host_id
            || offer.binding.source.boot_id != source.boot_id
            || offer.binding.sink.host_id != sink.host_id
            || offer.binding.sink.boot_id != sink.boot_id
        {
            return Err(PlanningAdviceRefusal::LineEndpointMismatch);
        }
        line_candidates.insert(key, vec![suggestion.line_id.clone()]);
    }

    Ok(AdvisedPlanningInputs {
        placements,
        line_candidates,
        evidence: PlanningAdviceEvidence {
            proposal_id: advice.proposal_id.clone(),
            request_identity: advice.request_identity.clone(),
            run_identity: advice.run_identity.clone(),
            checked_form_id: advice.checked_form_id.clone(),
            proposed_placements: advice.placements.len() as u16,
            used_placements: advice.placements.len() as u16,
            proposed_lines: advice.lines.len() as u16,
            used_lines: advice.lines.len() as u16,
        },
    })
}

fn current_host<'a>(
    hosts: &'a [HostAdvertisement],
    suggestion: &SuggestedPlacement,
) -> Result<&'a HostAdvertisement, PlanningAdviceRefusal> {
    let matching_host = hosts
        .iter()
        .filter(|host| host.host_id == suggestion.host_id)
        .collect::<Vec<_>>();
    if matching_host.is_empty() {
        return Err(PlanningAdviceRefusal::UnknownHost);
    }
    let matching_boot = matching_host
        .into_iter()
        .filter(|host| host.boot_id == suggestion.boot_id)
        .collect::<Vec<_>>();
    if matching_boot.is_empty() {
        return Err(PlanningAdviceRefusal::StaleBoot);
    }
    matching_boot
        .into_iter()
        .find(|host| host.offer_generation == suggestion.offer_generation)
        .ok_or(PlanningAdviceRefusal::StaleOfferGeneration)
}

fn selected_host<'a>(
    placements: &PlacementChoices,
    hosts: &'a [HostAdvertisement],
    gear_id: &GearId,
) -> Result<&'a HostAdvertisement, PlanningAdviceRefusal> {
    let choice = placements
        .by_gear
        .get(gear_id)
        .ok_or(PlanningAdviceRefusal::UnknownGear)?;
    let mut matching = hosts.iter().filter(|host| {
        host.host_id == choice.host_id
            && host
                .capabilities
                .iter()
                .any(|offer| offer.capability_id == choice.capability_id)
    });
    let selected = matching.next().ok_or(PlanningAdviceRefusal::UnknownHost)?;
    if matching.next().is_some() {
        return Err(PlanningAdviceRefusal::OrdinaryPlanningUnavailable);
    }
    Ok(selected)
}

fn validate_identity(identity: &str) -> Result<(), PlanningAdviceRefusal> {
    if identity.is_empty() {
        return Err(PlanningAdviceRefusal::EmptyIdentity);
    }
    if identity.len() > MAXIMUM_ADVICE_ID_BYTES {
        return Err(PlanningAdviceRefusal::IdentityTooLong);
    }
    Ok(())
}
