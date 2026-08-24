//! Bounded survival preferences over ordinary immutable Plan candidates.

use crate::prelude::*;
use conduit_core::{verify_plan, Plan, PlanId};
use core::cmp::Ordering;

pub const MAXIMUM_SURVIVAL_CANDIDATES: usize = 32;
pub const MAXIMUM_SURVIVAL_TRADEOFFS: usize = 8;
pub const MAXIMUM_SURVIVAL_POLICY_ID_BYTES: usize = 256;
pub const MAXIMUM_SCARCE_RESOURCE_REQUESTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivalPlanningMode {
    Normal,
    Survival,
}

/// Explicit lexicographic preferences. This is deliberately not one scalar score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SurvivalTradeoff {
    PreferFullProfile,
    MinimizeUnavailablePrerequisites,
    MinimizeSharedDependencyExposure,
    MinimizeHopCount,
    MinimizeLatency,
    MinimizeResourceUnits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurvivalPlanningPolicy {
    pub policy_id: String,
    pub revision: u64,
    pub mode: SurvivalPlanningMode,
    pub normal_maximum_hops: u16,
    pub normal_maximum_latency_us: u64,
    pub admit_costly_full_profile: bool,
    pub admit_reviewed_degradation: bool,
    pub tradeoffs: Vec<SurvivalTradeoff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurvivalCandidateDisposition {
    FullyCompatible,
    ReviewedDegraded {
        profile_id: String,
        admission_policy_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct SurvivalCandidate<'a> {
    pub plan: &'a Plan,
    pub semantic_profile: &'a str,
    pub disposition: SurvivalCandidateDisposition,
    pub current: bool,
    pub currently_available: bool,
    pub authority_admitted: bool,
    pub all_host_reservations_admitted: bool,
    pub unavailable_prerequisites: u16,
    pub shared_dependency_exposures: u16,
    pub hop_count: u16,
    pub estimated_item_latency_us: u64,
    pub resource_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurvivalCandidateEvidence {
    RejectedUnavailablePrerequisite,
    RejectedHardSemantic,
    RejectedAuthority,
    RejectedHostReservation,
    RejectedUnreviewedDegradation,
    RejectedNormalCostEnvelope,
    Admitted,
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurvivalPlanSelection {
    pub policy_id: String,
    pub policy_revision: u64,
    pub mode: SurvivalPlanningMode,
    pub selected_plan_id: PlanId,
    pub previous_plan_id: Option<PlanId>,
    pub fresh_plan: bool,
    pub selected_disposition: SurvivalCandidateDisposition,
    pub principal_tradeoffs: Vec<SurvivalTradeoff>,
    pub candidate_evidence: Vec<(PlanId, SurvivalCandidateEvidence)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivalPolicyRefusal {
    InvalidPolicy,
    InvalidCandidateSet,
    HardSemanticRequirementUnsatisfied,
    AuthorityUnavailable,
    HostReservationRefused,
    ReviewedDegradationRequired,
    NormalCostEnvelopeExceeded,
    NoAdmissibleCandidate,
}

pub fn select_plan_with_survival_policy(
    required_semantic_profile: &str,
    candidates: &[SurvivalCandidate<'_>],
    policy: &SurvivalPlanningPolicy,
) -> Result<SurvivalPlanSelection, SurvivalPolicyRefusal> {
    validate_policy(policy)?;
    validate_candidates(required_semantic_profile, candidates)?;

    let previous_plan_id = candidates
        .iter()
        .find(|candidate| candidate.current)
        .map(|candidate| candidate.plan.plan_id.clone());
    let mut evidence = Vec::with_capacity(candidates.len());
    let mut admitted = Vec::new();
    for candidate in candidates {
        let disposition = candidate_refusal(required_semantic_profile, candidate, policy);
        if disposition == SurvivalCandidateEvidence::Admitted {
            admitted.push(candidate);
        }
        evidence.push((candidate.plan.plan_id.clone(), disposition));
    }
    if admitted.is_empty() {
        return Err(classify_refusal(&evidence));
    }
    admitted.sort_by(|left, right| compare_candidates(left, right, &policy.tradeoffs));
    let selected = admitted[0];
    if let Some(record) = evidence
        .iter_mut()
        .find(|(plan_id, _)| plan_id == &selected.plan.plan_id)
    {
        record.1 = SurvivalCandidateEvidence::Selected;
    }
    let fresh_plan = previous_plan_id
        .as_ref()
        .is_none_or(|previous| previous != &selected.plan.plan_id);
    Ok(SurvivalPlanSelection {
        policy_id: policy.policy_id.clone(),
        policy_revision: policy.revision,
        mode: policy.mode,
        selected_plan_id: selected.plan.plan_id.clone(),
        previous_plan_id,
        fresh_plan,
        selected_disposition: selected.disposition.clone(),
        principal_tradeoffs: policy.tradeoffs.clone(),
        candidate_evidence: evidence,
    })
}

fn validate_policy(policy: &SurvivalPlanningPolicy) -> Result<(), SurvivalPolicyRefusal> {
    if policy.policy_id.is_empty()
        || policy.policy_id.len() > MAXIMUM_SURVIVAL_POLICY_ID_BYTES
        || policy.revision == 0
        || policy.tradeoffs.is_empty()
        || policy.tradeoffs.len() > MAXIMUM_SURVIVAL_TRADEOFFS
    {
        return Err(SurvivalPolicyRefusal::InvalidPolicy);
    }
    let unique = policy
        .tradeoffs
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>();
    if unique.len() != policy.tradeoffs.len()
        || (policy.mode == SurvivalPlanningMode::Normal
            && (policy.admit_costly_full_profile || policy.admit_reviewed_degradation))
    {
        return Err(SurvivalPolicyRefusal::InvalidPolicy);
    }
    Ok(())
}

fn validate_candidates(
    required_semantic_profile: &str,
    candidates: &[SurvivalCandidate<'_>],
) -> Result<(), SurvivalPolicyRefusal> {
    if required_semantic_profile.is_empty()
        || candidates.is_empty()
        || candidates.len() > MAXIMUM_SURVIVAL_CANDIDATES
        || candidates
            .iter()
            .any(|candidate| !verify_plan(candidate.plan))
    {
        return Err(SurvivalPolicyRefusal::InvalidCandidateSet);
    }
    let subject = &candidates[0].plan;
    if candidates.iter().any(|candidate| {
        candidate.plan.source_document_id != subject.source_document_id
            || candidate.plan.checked_form_id != subject.checked_form_id
            || matches!(
                &candidate.disposition,
                SurvivalCandidateDisposition::ReviewedDegraded {
                    profile_id,
                    admission_policy_id,
                } if profile_id.is_empty() || admission_policy_id.is_empty()
            )
    }) {
        return Err(SurvivalPolicyRefusal::InvalidCandidateSet);
    }
    let ids = candidates
        .iter()
        .map(|candidate| &candidate.plan.plan_id)
        .collect::<alloc::collections::BTreeSet<_>>();
    if ids.len() != candidates.len() || candidates.iter().filter(|item| item.current).count() > 1 {
        return Err(SurvivalPolicyRefusal::InvalidCandidateSet);
    }
    Ok(())
}

fn candidate_refusal(
    required_semantic_profile: &str,
    candidate: &SurvivalCandidate<'_>,
    policy: &SurvivalPlanningPolicy,
) -> SurvivalCandidateEvidence {
    if !candidate.currently_available {
        return SurvivalCandidateEvidence::RejectedUnavailablePrerequisite;
    }
    if candidate.semantic_profile != required_semantic_profile {
        return SurvivalCandidateEvidence::RejectedHardSemantic;
    }
    if !candidate.authority_admitted {
        return SurvivalCandidateEvidence::RejectedAuthority;
    }
    if !candidate.all_host_reservations_admitted {
        return SurvivalCandidateEvidence::RejectedHostReservation;
    }
    if matches!(
        candidate.disposition,
        SurvivalCandidateDisposition::ReviewedDegraded { .. }
    ) && !policy.admit_reviewed_degradation
    {
        return SurvivalCandidateEvidence::RejectedUnreviewedDegradation;
    }
    let outside_normal_cost = candidate.hop_count > policy.normal_maximum_hops
        || candidate.estimated_item_latency_us > policy.normal_maximum_latency_us;
    if outside_normal_cost
        && (policy.mode == SurvivalPlanningMode::Normal || !policy.admit_costly_full_profile)
    {
        return SurvivalCandidateEvidence::RejectedNormalCostEnvelope;
    }
    SurvivalCandidateEvidence::Admitted
}

fn classify_refusal(evidence: &[(PlanId, SurvivalCandidateEvidence)]) -> SurvivalPolicyRefusal {
    if evidence
        .iter()
        .any(|(_, item)| *item == SurvivalCandidateEvidence::RejectedHardSemantic)
    {
        SurvivalPolicyRefusal::HardSemanticRequirementUnsatisfied
    } else if evidence
        .iter()
        .any(|(_, item)| *item == SurvivalCandidateEvidence::RejectedAuthority)
    {
        SurvivalPolicyRefusal::AuthorityUnavailable
    } else if evidence
        .iter()
        .any(|(_, item)| *item == SurvivalCandidateEvidence::RejectedHostReservation)
    {
        SurvivalPolicyRefusal::HostReservationRefused
    } else if evidence
        .iter()
        .any(|(_, item)| *item == SurvivalCandidateEvidence::RejectedUnreviewedDegradation)
    {
        SurvivalPolicyRefusal::ReviewedDegradationRequired
    } else if evidence
        .iter()
        .any(|(_, item)| *item == SurvivalCandidateEvidence::RejectedNormalCostEnvelope)
    {
        SurvivalPolicyRefusal::NormalCostEnvelopeExceeded
    } else {
        SurvivalPolicyRefusal::NoAdmissibleCandidate
    }
}

fn compare_candidates(
    left: &SurvivalCandidate<'_>,
    right: &SurvivalCandidate<'_>,
    tradeoffs: &[SurvivalTradeoff],
) -> Ordering {
    for tradeoff in tradeoffs {
        let ordering = match tradeoff {
            SurvivalTradeoff::PreferFullProfile => {
                disposition_rank(&left.disposition).cmp(&disposition_rank(&right.disposition))
            }
            SurvivalTradeoff::MinimizeUnavailablePrerequisites => left
                .unavailable_prerequisites
                .cmp(&right.unavailable_prerequisites),
            SurvivalTradeoff::MinimizeSharedDependencyExposure => left
                .shared_dependency_exposures
                .cmp(&right.shared_dependency_exposures),
            SurvivalTradeoff::MinimizeHopCount => left.hop_count.cmp(&right.hop_count),
            SurvivalTradeoff::MinimizeLatency => left
                .estimated_item_latency_us
                .cmp(&right.estimated_item_latency_us),
            SurvivalTradeoff::MinimizeResourceUnits => {
                left.resource_units.cmp(&right.resource_units)
            }
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.plan.plan_id.cmp(&right.plan.plan_id)
}

fn disposition_rank(disposition: &SurvivalCandidateDisposition) -> u8 {
    match disposition {
        SurvivalCandidateDisposition::FullyCompatible => 0,
        SurvivalCandidateDisposition::ReviewedDegraded { .. } => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExplicitCriticality {
    Deferrable,
    Important,
    Essential,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadResourceRequest {
    pub workload_id: String,
    pub resource_units: u64,
    pub criticality: ExplicitCriticality,
    pub policy_source_id: String,
    pub policy_source_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScarceResourceDisposition {
    Reserved,
    RefusedCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScarceResourceDecision {
    pub workload_id: String,
    pub criticality: ExplicitCriticality,
    pub policy_source_id: String,
    pub policy_source_revision: u64,
    pub resource_units: u64,
    pub disposition: ScarceResourceDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScarceResourceTriage {
    pub capacity_units: u64,
    pub reserved_units: u64,
    pub decisions: Vec<ScarceResourceDecision>,
}

pub fn triage_scarce_resource(
    capacity_units: u64,
    requests: &[WorkloadResourceRequest],
) -> Result<ScarceResourceTriage, SurvivalPolicyRefusal> {
    if capacity_units == 0
        || requests.is_empty()
        || requests.len() > MAXIMUM_SCARCE_RESOURCE_REQUESTS
        || requests.iter().any(|request| {
            request.workload_id.is_empty()
                || request.policy_source_id.is_empty()
                || request.policy_source_revision == 0
                || request.resource_units == 0
        })
    {
        return Err(SurvivalPolicyRefusal::InvalidPolicy);
    }
    let ids = requests
        .iter()
        .map(|request| &request.workload_id)
        .collect::<alloc::collections::BTreeSet<_>>();
    if ids.len() != requests.len() {
        return Err(SurvivalPolicyRefusal::InvalidPolicy);
    }
    let mut ordered = requests.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .criticality
            .cmp(&left.criticality)
            .then_with(|| left.workload_id.cmp(&right.workload_id))
    });
    let mut reserved_units = 0_u64;
    let decisions = ordered
        .into_iter()
        .map(|request| {
            let fits = reserved_units
                .checked_add(request.resource_units)
                .is_some_and(|next| next <= capacity_units);
            if fits {
                reserved_units += request.resource_units;
            }
            ScarceResourceDecision {
                workload_id: request.workload_id.clone(),
                criticality: request.criticality,
                policy_source_id: request.policy_source_id.clone(),
                policy_source_revision: request.policy_source_revision,
                resource_units: request.resource_units,
                disposition: if fits {
                    ScarceResourceDisposition::Reserved
                } else {
                    ScarceResourceDisposition::RefusedCapacity
                },
            }
        })
        .collect();
    Ok(ScarceResourceTriage {
        capacity_units,
        reserved_units,
        decisions,
    })
}
