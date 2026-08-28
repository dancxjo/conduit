//! Finite admission of a reviewed recursive realization as full semantic recovery.

use crate::prelude::*;
use conduit_core::{verify_plan, Plan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecursiveRecoveryLimits {
    pub maximum_depth: u16,
    pub maximum_work: u32,
    pub maximum_candidates: u16,
    pub maximum_gears: u16,
    pub maximum_remote_connections: u16,
    pub maximum_item_latency_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveRecoveryCandidate<'a> {
    pub lost_direct_plan: &'a Plan,
    pub replacement_plan: &'a Plan,
    pub required_semantic_profile: &'a str,
    pub offered_semantic_profile: &'a str,
    pub offered_profile_is_reviewed_degradation: bool,
    pub direct_implementation_unavailable: bool,
    pub all_host_reservations_admitted: bool,
    pub all_required_authority_admitted: bool,
    pub expansion_depth: u16,
    pub search_work: u32,
    pub candidates_considered: u16,
    pub estimated_item_latency_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveRecoveryEvidence {
    pub semantic_profile: String,
    pub expanded_gear_count: u16,
    pub host_count: u16,
    pub remote_connection_count: u16,
    pub resource_binding_count: u16,
    pub authority_binding_count: u16,
    pub expansion_depth: u16,
    pub search_work: u32,
    pub candidates_considered: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursiveRecoveryRefusal {
    InvalidPlan,
    DirectImplementationStillAvailable,
    SemanticSubjectChanged,
    ReplacementPlanNotFresh,
    UnreviewedOrNonRecursiveRealization,
    SemanticProfileMismatch,
    SingleHostRealization,
    MissingExactLine,
    RequiresDegradedProfileAdmission,
    LatencyRequirementUnsatisfied,
    SearchBoundExceeded,
    ExpandedGearBoundExceeded,
    RemoteConnectionBoundExceeded,
    HostReservationRefused,
    AuthorityUnavailable,
    EvidenceOverflow,
}

pub fn prove_recursive_recovery(
    candidate: &RecursiveRecoveryCandidate<'_>,
    limits: RecursiveRecoveryLimits,
) -> Result<RecursiveRecoveryEvidence, RecursiveRecoveryRefusal> {
    let old = candidate.lost_direct_plan;
    let replacement = candidate.replacement_plan;
    if !verify_plan(old) || !verify_plan(replacement) {
        return Err(RecursiveRecoveryRefusal::InvalidPlan);
    }
    if !candidate.direct_implementation_unavailable {
        return Err(RecursiveRecoveryRefusal::DirectImplementationStillAvailable);
    }
    if old.source_document_id != replacement.source_document_id
        || old.checked_form_id != replacement.checked_form_id
    {
        return Err(RecursiveRecoveryRefusal::SemanticSubjectChanged);
    }
    if old.plan_id == replacement.plan_id {
        return Err(RecursiveRecoveryRefusal::ReplacementPlanNotFresh);
    }
    if replacement.realization_backs.is_empty() {
        return Err(RecursiveRecoveryRefusal::UnreviewedOrNonRecursiveRealization);
    }
    if candidate.required_semantic_profile != candidate.offered_semantic_profile {
        return Err(if candidate.offered_profile_is_reviewed_degradation {
            RecursiveRecoveryRefusal::RequiresDegradedProfileAdmission
        } else {
            RecursiveRecoveryRefusal::SemanticProfileMismatch
        });
    }
    if !candidate.all_host_reservations_admitted {
        return Err(RecursiveRecoveryRefusal::HostReservationRefused);
    }
    if !candidate.all_required_authority_admitted {
        return Err(RecursiveRecoveryRefusal::AuthorityUnavailable);
    }
    if candidate.expansion_depth > limits.maximum_depth
        || candidate.search_work > limits.maximum_work
        || candidate.candidates_considered > limits.maximum_candidates
    {
        return Err(RecursiveRecoveryRefusal::SearchBoundExceeded);
    }
    if candidate.estimated_item_latency_us > limits.maximum_item_latency_us {
        return Err(RecursiveRecoveryRefusal::LatencyRequirementUnsatisfied);
    }

    let gears = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .count();
    let remote = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| connection.selected_line.is_some())
        .count();
    if replacement.fragments.len() < 2 {
        return Err(RecursiveRecoveryRefusal::SingleHostRealization);
    }
    if remote == 0
        || replacement
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter(|connection| connection.selected_line.is_some())
            .any(|connection| connection.admitted_lines.is_empty())
    {
        return Err(RecursiveRecoveryRefusal::MissingExactLine);
    }
    if gears > usize::from(limits.maximum_gears) {
        return Err(RecursiveRecoveryRefusal::ExpandedGearBoundExceeded);
    }
    if remote > usize::from(limits.maximum_remote_connections) {
        return Err(RecursiveRecoveryRefusal::RemoteConnectionBoundExceeded);
    }

    let resources = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.resources.len())
        .sum::<usize>();
    let authority = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.authority.len())
        .sum::<usize>();
    Ok(RecursiveRecoveryEvidence {
        semantic_profile: candidate.required_semantic_profile.into(),
        expanded_gear_count: gears
            .try_into()
            .map_err(|_| RecursiveRecoveryRefusal::EvidenceOverflow)?,
        host_count: replacement
            .fragments
            .len()
            .try_into()
            .map_err(|_| RecursiveRecoveryRefusal::EvidenceOverflow)?,
        remote_connection_count: remote
            .try_into()
            .map_err(|_| RecursiveRecoveryRefusal::EvidenceOverflow)?,
        resource_binding_count: resources
            .try_into()
            .map_err(|_| RecursiveRecoveryRefusal::EvidenceOverflow)?,
        authority_binding_count: authority
            .try_into()
            .map_err(|_| RecursiveRecoveryRefusal::EvidenceOverflow)?,
        expansion_depth: candidate.expansion_depth,
        search_work: candidate.search_work,
        candidates_considered: candidate.candidates_considered,
    })
}
