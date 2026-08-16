use crate::prelude::*;
use crate::PlannerError;
use alloc::collections::{BTreeMap, BTreeSet};

pub const MAXIMUM_INCREMENTAL_CANDIDATES: usize = 32;
pub const MAXIMUM_CACHED_CANDIDATES: usize = 32;
pub const MAXIMUM_PLANNING_FACTS: usize = 128;
pub const MAXIMUM_CANDIDATE_DEPENDENCIES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactDomain {
    Semantic,
    Implementation,
    Host,
    Boot,
    Offer,
    Resource,
    Authority,
    Line,
    Policy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlanningFactKey {
    pub domain: FactDomain,
    pub identity: String,
}

impl PlanningFactKey {
    pub fn exact(domain: FactDomain, identity: impl Into<String>) -> Self {
        Self {
            domain,
            identity: identity.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningFact {
    pub key: PlanningFactKey,
    /// Exact revision/generation of current truth. Zero is never current.
    pub generation: u64,
    /// Digest or otherwise exact immutable identity of the fact content.
    pub content_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateStructure {
    pub candidate_id: String,
    pub semantic_contract_id: String,
    pub implementation_family_id: String,
    pub placement_id: String,
    pub dependencies: Vec<PlanningFactKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateEvaluationDisposition {
    Admitted,
    Rejected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateEvaluation {
    pub disposition: CandidateEvaluationDisposition,
    pub result_identity: String,
    pub total_cost: u64,
    /// Deterministic effort required to specialize this candidate against its
    /// fresh dependency basis.
    pub evaluation_work_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilityPolicy {
    pub previous_placement_id: Option<String>,
    /// Maximum reviewed cost disadvantage allowed to avoid gratuitous churn.
    pub maximum_cost_penalty: u64,
}

impl StabilityPolicy {
    pub const fn disabled() -> Self {
        Self {
            previous_placement_id: None,
            maximum_cost_penalty: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalCandidateEvidence {
    pub candidate_id: String,
    pub placement_id: String,
    pub evaluation: CandidateEvaluation,
    pub reused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IncrementalPlannerMetrics {
    pub search_nodes: u32,
    pub evaluated_candidates: u32,
    pub reused_candidates: u32,
    pub invalidated_candidates: u32,
    pub discarded_cache_entries: u32,
    pub evaluation_work_units: u64,
    /// Deterministic latency proxy: search nodes plus fresh evaluation work.
    pub logical_latency_work_units: u64,
    pub cache_entries: u32,
    pub cache_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalPlan {
    pub selected_candidate_id: String,
    pub selected_result_identity: String,
    pub selected_cost: u64,
    pub stability_preference_applied: bool,
    pub considered: Vec<IncrementalCandidateEvidence>,
    pub metrics: IncrementalPlannerMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedCandidate {
    structure: CandidateStructure,
    dependency_snapshots: Vec<(PlanningFactKey, u64, String)>,
    evaluation: CandidateEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalPlanner {
    capacity: usize,
    entries: Vec<CachedCandidate>,
}

impl IncrementalPlanner {
    pub fn new(capacity: usize) -> Result<Self, PlannerError> {
        if capacity == 0 || capacity > MAXIMUM_CACHED_CANDIDATES {
            return invalid("incremental cache capacity is zero or exceeds its finite bound");
        }
        Ok(Self {
            capacity,
            entries: Vec::with_capacity(capacity),
        })
    }

    pub fn retained_candidate_ids(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|entry| entry.structure.candidate_id.as_str())
            .collect()
    }

    pub fn discard(&mut self) {
        self.entries.clear();
    }

    pub fn plan<F>(
        &mut self,
        candidates: &[CandidateStructure],
        facts: &[PlanningFact],
        stability: &StabilityPolicy,
        mut evaluate: F,
    ) -> Result<IncrementalPlan, PlannerError>
    where
        F: FnMut(&CandidateStructure, &[PlanningFact]) -> CandidateEvaluation,
    {
        let fact_map = validate_inputs(candidates, facts, stability)?;
        let mut considered = Vec::with_capacity(candidates.len());
        let mut metrics = IncrementalPlannerMetrics {
            search_nodes: u32::try_from(candidates.len()).unwrap_or(u32::MAX),
            cache_capacity: u32::try_from(self.capacity).unwrap_or(u32::MAX),
            ..IncrementalPlannerMetrics::default()
        };

        for candidate in candidates {
            let dependency_snapshots = dependency_snapshots(candidate, &fact_map)?;
            let matching_index = self
                .entries
                .iter()
                .position(|entry| entry.structure.candidate_id == candidate.candidate_id);
            let cached = matching_index.and_then(|index| {
                let entry = &self.entries[index];
                (entry.structure == *candidate
                    && entry.dependency_snapshots == dependency_snapshots)
                    .then(|| entry.evaluation.clone())
            });
            let (evaluation, reused) = if let Some(evaluation) = cached {
                metrics.reused_candidates = metrics.reused_candidates.saturating_add(1);
                (evaluation, true)
            } else {
                if matching_index.is_some() {
                    metrics.invalidated_candidates =
                        metrics.invalidated_candidates.saturating_add(1);
                }
                let fresh_basis = candidate
                    .dependencies
                    .iter()
                    .map(|key| fact_map[key])
                    .cloned()
                    .collect::<Vec<_>>();
                let evaluation = evaluate(candidate, &fresh_basis);
                validate_evaluation(&evaluation)?;
                metrics.evaluated_candidates = metrics.evaluated_candidates.saturating_add(1);
                metrics.evaluation_work_units = metrics
                    .evaluation_work_units
                    .checked_add(evaluation.evaluation_work_units)
                    .ok_or_else(|| {
                        PlannerError::InvalidRealizationPolicy(
                            "incremental planning work overflowed".to_string(),
                        )
                    })?;
                self.retain(
                    CachedCandidate {
                        structure: candidate.clone(),
                        dependency_snapshots,
                        evaluation: evaluation.clone(),
                    },
                    matching_index,
                    &mut metrics,
                );
                (evaluation, false)
            };
            considered.push(IncrementalCandidateEvidence {
                candidate_id: candidate.candidate_id.clone(),
                placement_id: candidate.placement_id.clone(),
                evaluation,
                reused,
            });
        }

        let cheapest = considered
            .iter()
            .enumerate()
            .filter(|(_, evidence)| {
                evidence.evaluation.disposition == CandidateEvaluationDisposition::Admitted
            })
            .min_by_key(|(_, evidence)| {
                (
                    evidence.evaluation.total_cost,
                    evidence.candidate_id.as_str(),
                )
            })
            .map(|(index, _)| index)
            .ok_or_else(|| {
                PlannerError::HardRealizationRequirementUnsatisfied(
                    "no incremental candidate is admitted by fresh current truth".to_string(),
                )
            })?;
        let selected = stable_index(&considered, cheapest, stability);
        let stability_preference_applied = selected != cheapest;
        let winner = &considered[selected];
        metrics.logical_latency_work_units = u64::from(metrics.search_nodes)
            .checked_add(metrics.evaluation_work_units)
            .ok_or_else(|| {
                PlannerError::InvalidRealizationPolicy(
                    "incremental logical latency overflowed".to_string(),
                )
            })?;
        metrics.cache_entries = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        Ok(IncrementalPlan {
            selected_candidate_id: winner.candidate_id.clone(),
            selected_result_identity: winner.evaluation.result_identity.clone(),
            selected_cost: winner.evaluation.total_cost,
            stability_preference_applied,
            considered,
            metrics,
        })
    }

    fn retain(
        &mut self,
        entry: CachedCandidate,
        matching_index: Option<usize>,
        metrics: &mut IncrementalPlannerMetrics,
    ) {
        if let Some(index) = matching_index {
            self.entries[index] = entry;
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
            metrics.discarded_cache_entries = metrics.discarded_cache_entries.saturating_add(1);
        }
        self.entries.push(entry);
    }
}

pub fn plan_cold<F>(
    candidates: &[CandidateStructure],
    facts: &[PlanningFact],
    stability: &StabilityPolicy,
    evaluate: F,
) -> Result<IncrementalPlan, PlannerError>
where
    F: FnMut(&CandidateStructure, &[PlanningFact]) -> CandidateEvaluation,
{
    let mut planner = IncrementalPlanner::new(candidates.len().max(1))?;
    planner.plan(candidates, facts, stability, evaluate)
}

fn validate_inputs<'a>(
    candidates: &'a [CandidateStructure],
    facts: &'a [PlanningFact],
    stability: &StabilityPolicy,
) -> Result<BTreeMap<&'a PlanningFactKey, &'a PlanningFact>, PlannerError> {
    if candidates.is_empty() || candidates.len() > MAXIMUM_INCREMENTAL_CANDIDATES {
        return invalid("incremental candidate count is empty or exceeds its finite bound");
    }
    if facts.is_empty() || facts.len() > MAXIMUM_PLANNING_FACTS {
        return invalid("planning fact count is empty or exceeds its finite bound");
    }
    if stability
        .previous_placement_id
        .as_ref()
        .is_some_and(|identity| identity.is_empty())
    {
        return invalid("stable locality requires an exact prior placement identity");
    }
    let mut fact_map = BTreeMap::new();
    for fact in facts {
        if fact.key.identity.is_empty()
            || fact.generation == 0
            || fact.content_identity.is_empty()
            || fact_map.insert(&fact.key, fact).is_some()
        {
            return invalid("planning facts require unique exact identities and generations");
        }
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        let dependencies = candidate.dependencies.iter().collect::<BTreeSet<_>>();
        if candidate.candidate_id.is_empty()
            || candidate.semantic_contract_id.is_empty()
            || candidate.implementation_family_id.is_empty()
            || candidate.placement_id.is_empty()
            || !candidate_ids.insert(candidate.candidate_id.as_str())
            || candidate.dependencies.is_empty()
            || candidate.dependencies.len() > MAXIMUM_CANDIDATE_DEPENDENCIES
            || dependencies.len() != candidate.dependencies.len()
            || candidate
                .dependencies
                .iter()
                .any(|dependency| !fact_map.contains_key(dependency))
        {
            return invalid("incremental candidate structure or dependency basis is invalid");
        }
    }
    Ok(fact_map)
}

fn dependency_snapshots(
    candidate: &CandidateStructure,
    facts: &BTreeMap<&PlanningFactKey, &PlanningFact>,
) -> Result<Vec<(PlanningFactKey, u64, String)>, PlannerError> {
    candidate
        .dependencies
        .iter()
        .map(|key| {
            facts
                .get(key)
                .map(|fact| (key.clone(), fact.generation, fact.content_identity.clone()))
                .ok_or_else(|| {
                    PlannerError::InvalidRealizationPolicy(
                        "candidate dependency disappeared from current truth".to_string(),
                    )
                })
        })
        .collect()
}

fn validate_evaluation(evaluation: &CandidateEvaluation) -> Result<(), PlannerError> {
    if evaluation.result_identity.is_empty() || evaluation.evaluation_work_units == 0 {
        return invalid("candidate evaluation requires exact result identity and finite work");
    }
    if matches!(
        &evaluation.disposition,
        CandidateEvaluationDisposition::Rejected(reason) if reason.is_empty()
    ) {
        return invalid("candidate rejection requires a machine-readable reason");
    }
    Ok(())
}

fn stable_index(
    considered: &[IncrementalCandidateEvidence],
    cheapest: usize,
    stability: &StabilityPolicy,
) -> usize {
    let Some(previous) = &stability.previous_placement_id else {
        return cheapest;
    };
    let Some((index, evidence)) = considered.iter().enumerate().find(|(_, evidence)| {
        &evidence.placement_id == previous
            && evidence.evaluation.disposition == CandidateEvaluationDisposition::Admitted
    }) else {
        return cheapest;
    };
    let allowed = considered[cheapest]
        .evaluation
        .total_cost
        .saturating_add(stability.maximum_cost_penalty);
    if evidence.evaluation.total_cost <= allowed {
        index
    } else {
        cheapest
    }
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(detail.to_string()))
}
