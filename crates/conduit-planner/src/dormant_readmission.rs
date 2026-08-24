use crate::observations::validate_resource_observations;
use crate::prelude::*;
use alloc::collections::BTreeSet;
use conduit_core::{
    AuthorityGrant, BootId, CapabilityId, GearId, HostAdvertisement, HostId, ImplementationId,
    LineAvailability, LineContract, LineId, LineOffer, OfferGeneration, Plan, PlanId,
    ResourceObservation, SignId,
};
use conduit_form::CheckedGear;

pub const MAXIMUM_DORMANT_ABSENT_GENERATIONS: usize = 32;
pub const MAXIMUM_DORMANT_REQUIRED_LINES: usize = 16;
pub const MAXIMUM_DORMANT_SIGNS: usize = 64;
pub const MAXIMUM_DORMANT_ID_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DormantEquipmentHistory {
    pub body_membership_id: String,
    pub host_id: HostId,
    pub last_observed_boot_id: BootId,
    pub last_offer_generation: OfferGeneration,
    pub absent_planning_generations: Vec<u64>,
    pub last_selected_plan_id: Option<PlanId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredDormantLine {
    pub line_id: LineId,
    pub contract: LineContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDormantCandidate {
    pub body_membership_id: String,
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub required_lines: Vec<RequiredDormantLine>,
    pub resource_observation_signs: Vec<SignId>,
    pub line_observation_signs: Vec<SignId>,
    pub authority_grant_ids: Vec<conduit_core::AuthorityGrantId>,
    pub unused_before: bool,
    pub available_now: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DormantReadmissionEvidence {
    pub candidate: CurrentDormantCandidate,
    pub previous_plan_id: PlanId,
    pub plan_id: PlanId,
    pub selected_because_preferred_path_is_gone: bool,
    pub historical_boot_reused: bool,
    pub historical_authority_restored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DormantReadmissionRefusal {
    InvalidHistory,
    IncompatibleHostProtocol,
    HostMismatch,
    StaleBoot,
    StaleOfferGeneration,
    MissingCurrentCapability,
    IncompatibleContractRevision,
    MissingCurrentResourceObservation,
    StaleCurrentResourceObservation,
    MissingCurrentLine,
    StaleCurrentLine,
    IncompatibleLineContract,
    MissingCurrentAuthority,
    InvalidPlan,
    FormChanged,
    PlanReused,
    PlacementMismatch,
    LineNotAdmitted,
}

/// Revalidates a historically known Host entirely from fresh current truth.
/// History can identify what to inspect, but contributes no capability,
/// resource, Line, or authority fact to the returned candidate.
pub fn observe_dormant_candidate(
    gear: &CheckedGear,
    history: &DormantEquipmentHistory,
    current_host: &HostAdvertisement,
    resource_observations: &[ResourceObservation],
    required_lines: &[RequiredDormantLine],
    line_offers: &[LineOffer],
    authority_grants: &[AuthorityGrant],
) -> Result<CurrentDormantCandidate, DormantReadmissionRefusal> {
    validate_history(history)?;
    if current_host.protocol_version != conduit_core::PROTOCOL_VERSION {
        return Err(DormantReadmissionRefusal::IncompatibleHostProtocol);
    }
    if current_host.host_id != history.host_id {
        return Err(DormantReadmissionRefusal::HostMismatch);
    }
    if current_host.boot_id == history.last_observed_boot_id {
        return Err(DormantReadmissionRefusal::StaleBoot);
    }
    if current_host.offer_generation.0 <= history.last_offer_generation.0 {
        return Err(DormantReadmissionRefusal::StaleOfferGeneration);
    }
    let offer = current_host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id == gear.kind_id)
        .ok_or(DormantReadmissionRefusal::MissingCurrentCapability)?;
    if offer.kind_contract_revision != gear.kind_contract_revision
        || offer.checked_face() != gear.checked_face()
    {
        return Err(DormantReadmissionRefusal::IncompatibleContractRevision);
    }
    validate_resources(current_host, offer, resource_observations)?;
    let (required_lines, line_observation_signs) =
        validate_lines(current_host, required_lines, line_offers)?;
    let authority_grant_ids = validate_authority(current_host, offer, authority_grants)?;
    let resource_observation_signs = resource_observations
        .iter()
        .filter(|observation| observation.host_id == current_host.host_id)
        .map(|observation| observation.sign_id.clone())
        .collect::<Vec<_>>();
    if resource_observation_signs.len() + line_observation_signs.len() > MAXIMUM_DORMANT_SIGNS {
        return Err(DormantReadmissionRefusal::InvalidHistory);
    }
    Ok(CurrentDormantCandidate {
        body_membership_id: history.body_membership_id.clone(),
        gear_id: gear.gear_id.clone(),
        host_id: current_host.host_id.clone(),
        boot_id: current_host.boot_id.clone(),
        offer_generation: current_host.offer_generation,
        capability_id: offer.capability_id.clone(),
        implementation_id: offer.implementation.implementation_id.clone(),
        required_lines,
        resource_observation_signs,
        line_observation_signs,
        authority_grant_ids,
        unused_before: history.last_selected_plan_id.is_none(),
        available_now: true,
    })
}

pub fn prove_dormant_readmission(
    previous_plan: &Plan,
    plan: &Plan,
    candidate: CurrentDormantCandidate,
) -> Result<DormantReadmissionEvidence, DormantReadmissionRefusal> {
    if !conduit_core::verify_plan(previous_plan) || !conduit_core::verify_plan(plan) {
        return Err(DormantReadmissionRefusal::InvalidPlan);
    }
    if previous_plan.source_document_id != plan.source_document_id
        || previous_plan.checked_form_id != plan.checked_form_id
        || previous_plan.expanded_form_id != plan.expanded_form_id
    {
        return Err(DormantReadmissionRefusal::FormChanged);
    }
    if previous_plan.plan_id == plan.plan_id {
        return Err(DormantReadmissionRefusal::PlanReused);
    }
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == candidate.gear_id)
        .ok_or(DormantReadmissionRefusal::PlacementMismatch)?;
    if placement.host_id != candidate.host_id
        || placement.boot_id != candidate.boot_id
        || placement.offer_generation != candidate.offer_generation
        || placement.capability_id != candidate.capability_id
        || placement.implementation_id != candidate.implementation_id
        || candidate.authority_grant_ids.iter().any(|grant_id| {
            placement
                .authority
                .iter()
                .all(|binding| &binding.grant_id != grant_id)
        })
    {
        return Err(DormantReadmissionRefusal::PlacementMismatch);
    }
    if candidate.required_lines.iter().any(|required| {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .flat_map(|connection| &connection.admitted_lines)
            .all(|line| line.line_id != required.line_id || line.contract != required.contract)
    }) {
        return Err(DormantReadmissionRefusal::LineNotAdmitted);
    }
    Ok(DormantReadmissionEvidence {
        previous_plan_id: previous_plan.plan_id.clone(),
        plan_id: plan.plan_id.clone(),
        candidate,
        selected_because_preferred_path_is_gone: true,
        historical_boot_reused: false,
        historical_authority_restored: false,
    })
}

fn validate_history(history: &DormantEquipmentHistory) -> Result<(), DormantReadmissionRefusal> {
    let valid_id = |value: &str| !value.is_empty() && value.len() <= MAXIMUM_DORMANT_ID_BYTES;
    if !valid_id(&history.body_membership_id)
        || history.host_id.as_str().is_empty()
        || history.last_observed_boot_id.as_str().is_empty()
        || history.last_offer_generation.0 == 0
        || history.absent_planning_generations.len() < 2
        || history.absent_planning_generations.len() > MAXIMUM_DORMANT_ABSENT_GENERATIONS
        || history
            .absent_planning_generations
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(DormantReadmissionRefusal::InvalidHistory);
    }
    Ok(())
}

fn validate_resources(
    host: &HostAdvertisement,
    offer: &conduit_core::CapabilityOffer,
    observations: &[ResourceObservation],
) -> Result<(), DormantReadmissionRefusal> {
    validate_resource_observations(core::slice::from_ref(host), observations)
        .map_err(|_| DormantReadmissionRefusal::StaleCurrentResourceObservation)?;
    for requirement in &offer.resource_requirements {
        let current = observations.iter().any(|observation| {
            observation.host_id == host.host_id
                && observation.boot_id == host.boot_id
                && observation.offer_generation == host.offer_generation
                && observation.class_id == requirement.class_id
                && observation.health == conduit_core::ResourceHealth::Ready
                && observation.unreserved_units >= requirement.units
        });
        if !current {
            return Err(DormantReadmissionRefusal::MissingCurrentResourceObservation);
        }
    }
    Ok(())
}

fn validate_lines(
    host: &HostAdvertisement,
    required: &[RequiredDormantLine],
    offers: &[LineOffer],
) -> Result<(Vec<RequiredDormantLine>, Vec<SignId>), DormantReadmissionRefusal> {
    if required.is_empty() || required.len() > MAXIMUM_DORMANT_REQUIRED_LINES {
        return Err(DormantReadmissionRefusal::MissingCurrentLine);
    }
    let mut ids = BTreeSet::new();
    let mut signs = Vec::with_capacity(required.len());
    for requirement in required {
        if !ids.insert(requirement.line_id.clone()) {
            return Err(DormantReadmissionRefusal::MissingCurrentLine);
        }
        let offer = offers
            .iter()
            .find(|offer| offer.line_id == requirement.line_id)
            .ok_or(DormantReadmissionRefusal::MissingCurrentLine)?;
        if !offer.validate_sign_identity()
            || offer.availability.availability != LineAvailability::Ready
            || (offer.binding.source.host_id != host.host_id
                && offer.binding.sink.host_id != host.host_id)
            || (offer.binding.source.host_id == host.host_id
                && offer.binding.source.boot_id != host.boot_id)
            || (offer.binding.sink.host_id == host.host_id
                && offer.binding.sink.boot_id != host.boot_id)
        {
            return Err(DormantReadmissionRefusal::StaleCurrentLine);
        }
        if offer.contract != requirement.contract {
            return Err(DormantReadmissionRefusal::IncompatibleLineContract);
        }
        signs.push(offer.availability.sign_id.clone());
    }
    Ok((required.to_vec(), signs))
}

fn validate_authority(
    host: &HostAdvertisement,
    offer: &conduit_core::CapabilityOffer,
    grants: &[AuthorityGrant],
) -> Result<Vec<conduit_core::AuthorityGrantId>, DormantReadmissionRefusal> {
    let mut result = Vec::with_capacity(offer.authority_requirements.len());
    for requirement in &offer.authority_requirements {
        let grant = grants.iter().find(|grant| {
            grant.contract_id == requirement.contract_id
                && grant.host_operation_contract_id == requirement.host_operation_contract_id
                && grant.subject_kind == requirement.subject_kind
                && grant.host_id == host.host_id
                && grant.boot_id == host.boot_id
                && grant.capability_id == offer.capability_id
        });
        let Some(grant) = grant else {
            return Err(DormantReadmissionRefusal::MissingCurrentAuthority);
        };
        result.push(grant.grant_id.clone());
    }
    Ok(result)
}
