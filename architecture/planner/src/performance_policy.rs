use crate::prelude::*;
use crate::{ObservationProvenance, PlannerError, PolicySourceRevision};
use alloc::collections::BTreeSet;
use conduit_core::{CheckedFormId, HostId, SignId};

pub const MAXIMUM_PERFORMANCE_CANDIDATES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceIntent {
    Interactive,
    Streaming,
    ThroughputBatch,
    Background,
    BoundedResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformancePolicy {
    pub source: PolicySourceRevision,
    pub intent: PerformanceIntent,
    pub maximum_startup_us: Option<u64>,
    pub maximum_item_latency_us: Option<u64>,
    pub minimum_throughput_items_per_second: Option<u64>,
    pub maximum_jitter_us: Option<u64>,
    pub maximum_bounded_response_us: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceProfileObservation {
    pub candidate_id: String,
    pub startup_us: u64,
    pub item_latency_us: u64,
    pub throughput_items_per_second: u64,
    pub jitter_us: u64,
    /// `None` means this realization makes no bounded-response guarantee.
    pub bounded_response_us: Option<u64>,
    pub transport_work_units: u64,
    pub compute_work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceCandidate {
    pub candidate_id: String,
    pub selected_hosts: Vec<HostId>,
    pub profile: PerformanceProfileObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerformanceCandidateDisposition {
    Admitted,
    Rejected(String),
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceCandidateEvidence {
    pub candidate_id: String,
    pub disposition: PerformanceCandidateDisposition,
    pub selected_hosts: Vec<HostId>,
    pub startup_us: u64,
    pub item_latency_us: u64,
    pub throughput_items_per_second: u64,
    pub jitter_us: u64,
    pub bounded_response_us: Option<u64>,
    pub transport_work_units: u64,
    pub compute_work_units: u64,
    pub supporting_sign_id: SignId,
    pub policy_source: PolicySourceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformancePolicySelection {
    pub checked_form_id: CheckedFormId,
    pub selected_candidate_id: String,
    pub policy: PerformancePolicy,
    pub considered: Vec<PerformanceCandidateEvidence>,
}

impl PerformancePolicySelection {
    pub fn explain(&self) -> String {
        let winner = self
            .considered
            .iter()
            .find(|item| item.disposition == PerformanceCandidateDisposition::Selected)
            .expect("performance selection always has one winner");
        format!(
            "policy '{}' revision {} selected '{}' for {:?}: startup={}us item-latency={}us throughput={}/s jitter={}us hosts={}",
            self.policy.source.source_id.as_str(),
            self.policy.source.revision,
            winner.candidate_id,
            self.policy.intent,
            winner.startup_us,
            winner.item_latency_us,
            winner.throughput_items_per_second,
            winner.jitter_us,
            winner.selected_hosts.len()
        )
    }
}

pub fn select_performance_candidate(
    checked_form_id: CheckedFormId,
    candidates: &[PerformanceCandidate],
    policy: &PerformancePolicy,
    now_ms: u64,
) -> Result<PerformancePolicySelection, PlannerError> {
    validate(candidates, policy, now_ms)?;
    let mut considered = candidates
        .iter()
        .map(|candidate| evaluate(candidate, policy))
        .collect::<Vec<_>>();
    let selected = considered
        .iter()
        .enumerate()
        .filter(|(_, evidence)| evidence.disposition == PerformanceCandidateDisposition::Admitted)
        .min_by_key(|(_, evidence)| ranking_key(evidence, policy.intent))
        .map(|(index, _)| index)
        .ok_or_else(|| {
            PlannerError::HardRealizationRequirementUnsatisfied(
                "no candidate satisfies the explicit performance policy".to_string(),
            )
        })?;
    considered[selected].disposition = PerformanceCandidateDisposition::Selected;
    Ok(PerformancePolicySelection {
        checked_form_id,
        selected_candidate_id: considered[selected].candidate_id.clone(),
        policy: policy.clone(),
        considered,
    })
}

fn validate(
    candidates: &[PerformanceCandidate],
    policy: &PerformancePolicy,
    now_ms: u64,
) -> Result<(), PlannerError> {
    if candidates.is_empty() || candidates.len() > MAXIMUM_PERFORMANCE_CANDIDATES {
        return invalid("performance candidate count is empty or exceeds its finite bound");
    }
    if policy.source.source_id.as_str().is_empty() || policy.source.revision == 0 {
        return invalid("performance policy requires an exact reviewed source revision");
    }
    let mut candidate_ids = BTreeSet::new();
    let mut signs = BTreeSet::new();
    for candidate in candidates {
        let profile = &candidate.profile;
        if candidate.candidate_id.is_empty()
            || profile.candidate_id != candidate.candidate_id
            || !candidate_ids.insert(candidate.candidate_id.as_str())
            || candidate.selected_hosts.is_empty()
            || profile.throughput_items_per_second == 0
            || profile.provenance.sign_id.as_str().is_empty()
            || !signs.insert(&profile.provenance.sign_id)
            || profile.provenance.source.is_empty()
            || profile.provenance.observed_at_ms > now_ms
            || now_ms > profile.provenance.valid_until_ms
        {
            return invalid("performance profile identity, capacity, or provenance is invalid");
        }
        let unique_hosts = candidate.selected_hosts.iter().collect::<BTreeSet<_>>();
        if unique_hosts.len() != candidate.selected_hosts.len() {
            return invalid("one performance candidate may not count a Host twice");
        }
    }
    Ok(())
}

fn evaluate(
    candidate: &PerformanceCandidate,
    policy: &PerformancePolicy,
) -> PerformanceCandidateEvidence {
    let profile = &candidate.profile;
    let mut evidence = PerformanceCandidateEvidence {
        candidate_id: candidate.candidate_id.clone(),
        disposition: PerformanceCandidateDisposition::Admitted,
        selected_hosts: candidate.selected_hosts.clone(),
        startup_us: profile.startup_us,
        item_latency_us: profile.item_latency_us,
        throughput_items_per_second: profile.throughput_items_per_second,
        jitter_us: profile.jitter_us,
        bounded_response_us: profile.bounded_response_us,
        transport_work_units: profile.transport_work_units,
        compute_work_units: profile.compute_work_units,
        supporting_sign_id: profile.provenance.sign_id.clone(),
        policy_source: policy.source.clone(),
    };
    let refusal = if policy
        .maximum_startup_us
        .is_some_and(|limit| profile.startup_us > limit)
    {
        Some("startup exceeds the reviewed maximum")
    } else if policy
        .maximum_item_latency_us
        .is_some_and(|limit| profile.item_latency_us > limit)
    {
        Some("item latency exceeds the reviewed maximum")
    } else if policy
        .minimum_throughput_items_per_second
        .is_some_and(|minimum| profile.throughput_items_per_second < minimum)
    {
        Some("throughput is below the reviewed minimum")
    } else if policy
        .maximum_jitter_us
        .is_some_and(|limit| profile.jitter_us > limit)
    {
        Some("jitter exceeds the reviewed maximum")
    } else if let Some(limit) = policy.maximum_bounded_response_us {
        match profile.bounded_response_us {
            Some(bound) if bound <= limit => None,
            Some(_) => Some("bounded response exceeds the reviewed maximum"),
            None => Some("candidate offers no bounded-response guarantee"),
        }
    } else {
        None
    };
    if let Some(reason) = refusal {
        evidence.disposition = PerformanceCandidateDisposition::Rejected(reason.to_string());
    }
    evidence
}

fn ranking_key(
    evidence: &PerformanceCandidateEvidence,
    intent: PerformanceIntent,
) -> (u64, u64, u64, u64, &str) {
    match intent {
        PerformanceIntent::Interactive => (
            evidence.item_latency_us,
            evidence.startup_us,
            evidence.transport_work_units,
            u64::MAX - evidence.throughput_items_per_second,
            evidence.candidate_id.as_str(),
        ),
        PerformanceIntent::Streaming => (
            evidence.item_latency_us,
            u64::MAX - evidence.throughput_items_per_second,
            evidence.jitter_us,
            evidence.transport_work_units,
            evidence.candidate_id.as_str(),
        ),
        PerformanceIntent::ThroughputBatch => (
            u64::MAX - evidence.throughput_items_per_second,
            evidence.compute_work_units,
            evidence.transport_work_units,
            evidence.startup_us,
            evidence.candidate_id.as_str(),
        ),
        PerformanceIntent::Background => (
            evidence.compute_work_units,
            evidence.transport_work_units,
            evidence.startup_us,
            u64::MAX - evidence.throughput_items_per_second,
            evidence.candidate_id.as_str(),
        ),
        PerformanceIntent::BoundedResponse => (
            evidence.bounded_response_us.unwrap_or(u64::MAX),
            evidence.jitter_us,
            evidence.item_latency_us,
            evidence.transport_work_units,
            evidence.candidate_id.as_str(),
        ),
    }
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(detail.to_string()))
}
