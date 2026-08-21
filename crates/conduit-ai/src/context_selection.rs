//! Typed reranking and finite structured context selection.

use alloc::{string::String, vec::Vec};
use conduit_core::TemporalRelation;

use crate::{
    ChunkIdentity, EntityBoundary, ExtractedSourceValue, RerankedCandidate, RerankingProofClass,
    RetrievalContribution, TemporalContext, TemporalProvenance, TemporalSource, TemporalValidity,
    MAXIMUM_CONTEXT_ITEMS, MAXIMUM_HYBRID_OUTPUT_CANDIDATES, MAXIMUM_RAG_IDENTITY_BYTES,
};

pub const MAXIMUM_CONTEXT_SELECTION_WORK_UNITS: u32 = 1_048_576;
pub const MAXIMUM_CONTEXT_BYTES: u32 = 1_048_576;
pub const MAXIMUM_CONTEXT_SELECTION_TOKENS: u32 = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextTemporalEvidence {
    pub evidence_identity: String,
    pub provenance: TemporalProvenance,
    pub source: TemporalSource,
    pub boundary: Option<EntityBoundary>,
    pub context: TemporalContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCandidate {
    pub reranked: RerankedCandidate,
    pub reranking_policy_identity: String,
    pub reranking_proof_class: RerankingProofClass,
    pub temporal: Option<ContextTemporalEvidence>,
    /// An exact reviewed grouping fact, never inferred from content at selection time.
    pub redundancy_group: Option<String>,
    pub token_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextRedundancyPolicy {
    KeepAll,
    OnePerReviewedGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextOrderingPolicy {
    Reranked,
    ChronologicalOldestFirst,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSelectionPolicy {
    pub identity: String,
    pub token_accounting_profile: String,
    pub redundancy: ContextRedundancyPolicy,
    pub ordering: ContextOrderingPolicy,
    pub maximum_items: u16,
    pub maximum_bytes: u32,
    pub maximum_tokens: u32,
    pub maximum_work_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedContextCost {
    pub bytes: u32,
    pub tokens: u32,
    pub work_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedContextRationale {
    Reranked,
    TemporalChronology,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedContextItem {
    pub reranked: RerankedCandidate,
    pub reranking_policy_identity: String,
    pub reranking_proof_class: RerankingProofClass,
    pub temporal: Option<ContextTemporalEvidence>,
    pub redundancy_group: Option<String>,
    pub rationale: SelectedContextRationale,
    pub budget: SelectedContextCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextOmissionReason {
    ReviewedRedundancy,
    ItemBudget,
    ByteBudget,
    TokenBudget,
    WorkBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextOmission {
    pub chunk_identity: ChunkIdentity,
    pub reason: ContextOmissionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextSelectionDisposition {
    Complete,
    Omitted { candidates: Vec<ContextOmission> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredContext {
    pub policy_identity: String,
    pub token_accounting_profile: String,
    pub redundancy: ContextRedundancyPolicy,
    pub ordering: ContextOrderingPolicy,
    pub items: Vec<SelectedContextItem>,
    pub disposition: ContextSelectionDisposition,
    pub used: SelectedContextCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSelectionRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    EmptyCandidates,
    CandidateLimitExceeded,
    InvalidBound,
    InvalidCandidate,
    DuplicateCandidate,
    ArithmeticOverflow,
    InvalidTemporalEvidence,
    TemporalEvidenceNotContributed,
    MissingTemporalEvidence,
    MissingRedundancyGroup,
    EmptyTokenCost,
    NoSelectedContext,
}

impl ContextSelectionPolicy {
    pub fn select(
        &self,
        candidates: &[ContextCandidate],
    ) -> Result<StructuredContext, ContextSelectionRefusal> {
        self.validate(candidates)?;
        let mut ordered = candidates.to_vec();
        ordered.sort_by(|left, right| match self.ordering {
            ContextOrderingPolicy::Reranked => left
                .reranked
                .reranked_rank
                .cmp(&right.reranked.reranked_rank),
            ContextOrderingPolicy::ChronologicalOldestFirst => temporal_instant(left)
                .cmp(&temporal_instant(right))
                .then_with(|| {
                    left.reranked
                        .reranked_rank
                        .cmp(&right.reranked.reranked_rank)
                }),
        });
        let mut items = Vec::new();
        let mut omissions = Vec::new();
        let mut used = SelectedContextCost {
            bytes: 0,
            tokens: 0,
            work_units: 0,
        };
        let mut groups: Vec<String> = Vec::new();
        for candidate in ordered {
            let identity = candidate.reranked.candidate.chunk.identity;
            if self.redundancy == ContextRedundancyPolicy::OnePerReviewedGroup {
                let group = candidate
                    .redundancy_group
                    .as_ref()
                    .ok_or(ContextSelectionRefusal::MissingRedundancyGroup)?;
                if groups.contains(group) {
                    omissions.push(ContextOmission {
                        chunk_identity: identity,
                        reason: ContextOmissionReason::ReviewedRedundancy,
                    });
                    continue;
                }
                groups.push(group.clone());
            }
            let cost = SelectedContextCost {
                bytes: extracted_value_bytes(&candidate.reranked.candidate.chunk.value)?,
                tokens: candidate.token_count,
                work_units: 1,
            };
            let reason = budget_refusal(self, &used, &cost, items.len())?;
            if let Some(reason) = reason {
                omissions.push(ContextOmission {
                    chunk_identity: identity,
                    reason,
                });
                continue;
            }
            used.bytes += cost.bytes;
            used.tokens += cost.tokens;
            used.work_units += cost.work_units;
            items.push(SelectedContextItem {
                reranked: candidate.reranked,
                reranking_policy_identity: candidate.reranking_policy_identity,
                reranking_proof_class: candidate.reranking_proof_class,
                temporal: candidate.temporal,
                redundancy_group: candidate.redundancy_group,
                rationale: match self.ordering {
                    ContextOrderingPolicy::Reranked => SelectedContextRationale::Reranked,
                    ContextOrderingPolicy::ChronologicalOldestFirst => {
                        SelectedContextRationale::TemporalChronology
                    }
                },
                budget: cost,
            });
        }
        if items.is_empty() {
            return Err(ContextSelectionRefusal::NoSelectedContext);
        }
        Ok(StructuredContext {
            policy_identity: self.identity.clone(),
            token_accounting_profile: self.token_accounting_profile.clone(),
            redundancy: self.redundancy,
            ordering: self.ordering,
            items,
            disposition: if omissions.is_empty() {
                ContextSelectionDisposition::Complete
            } else {
                ContextSelectionDisposition::Omitted {
                    candidates: omissions,
                }
            },
            used,
        })
    }

    fn validate(&self, candidates: &[ContextCandidate]) -> Result<(), ContextSelectionRefusal> {
        validate_identity(&self.identity)?;
        validate_identity(&self.token_accounting_profile)?;
        if candidates.is_empty() {
            return Err(ContextSelectionRefusal::EmptyCandidates);
        }
        if candidates.len() > MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize {
            return Err(ContextSelectionRefusal::CandidateLimitExceeded);
        }
        if self.maximum_items == 0
            || usize::from(self.maximum_items) > MAXIMUM_CONTEXT_ITEMS
            || self.maximum_bytes == 0
            || self.maximum_bytes > MAXIMUM_CONTEXT_BYTES
            || self.maximum_tokens == 0
            || self.maximum_tokens > MAXIMUM_CONTEXT_SELECTION_TOKENS
            || self.maximum_work_units == 0
            || self.maximum_work_units > MAXIMUM_CONTEXT_SELECTION_WORK_UNITS
        {
            return Err(ContextSelectionRefusal::InvalidBound);
        }
        for (index, candidate) in candidates.iter().enumerate() {
            validate_identity(&candidate.reranking_policy_identity)?;
            candidate
                .reranked
                .candidate
                .chunk
                .validate()
                .map_err(|_| ContextSelectionRefusal::InvalidCandidate)?;
            if candidate.token_count == 0 {
                return Err(ContextSelectionRefusal::EmptyTokenCost);
            }
            if candidate.reranked.reranked_rank == 0
                || usize::from(candidate.reranked.reranked_rank) > candidates.len()
                || candidate.reranked.original_rank == 0
                || candidate.reranked.candidate.contributions.is_empty()
            {
                return Err(ContextSelectionRefusal::InvalidCandidate);
            }
            if candidates[index + 1..].iter().any(|other| {
                other.reranked.candidate.chunk.identity
                    == candidate.reranked.candidate.chunk.identity
                    || other.reranked.reranked_rank == candidate.reranked.reranked_rank
            }) {
                return Err(ContextSelectionRefusal::DuplicateCandidate);
            }
            if let Some(group) = &candidate.redundancy_group {
                validate_identity(group)?;
            }
            if self.ordering == ContextOrderingPolicy::ChronologicalOldestFirst
                && candidate.temporal.is_none()
            {
                return Err(ContextSelectionRefusal::MissingTemporalEvidence);
            }
            validate_temporal(candidate)?;
        }
        Ok(())
    }
}

fn validate_temporal(candidate: &ContextCandidate) -> Result<(), ContextSelectionRefusal> {
    let Some(temporal) = &candidate.temporal else {
        if candidate
            .reranked
            .candidate
            .contributions
            .iter()
            .any(|path| path.temporal_evidence_identity.is_some())
        {
            return Err(ContextSelectionRefusal::MissingTemporalEvidence);
        }
        return Ok(());
    };
    validate_identity(&temporal.evidence_identity)?;
    temporal
        .provenance
        .validate()
        .map_err(|_| ContextSelectionRefusal::InvalidTemporalEvidence)?;
    let relation = temporal
        .provenance
        .relation(temporal.source)
        .map_err(|_| ContextSelectionRefusal::InvalidTemporalEvidence)?;
    if relation != temporal.context.relation
        || temporal.source != temporal.context.source
        || temporal.context.validity == TemporalValidity::UnknownWhetherCurrent
            && temporal.provenance.valid_from.is_some()
        || !candidate
            .reranked
            .candidate
            .contributions
            .iter()
            .any(|path| {
                path.temporal_evidence_identity.as_deref()
                    == Some(temporal.evidence_identity.as_str())
            })
    {
        return Err(ContextSelectionRefusal::TemporalEvidenceNotContributed);
    }
    Ok(())
}

fn temporal_instant(candidate: &ContextCandidate) -> u64 {
    candidate
        .temporal
        .as_ref()
        .and_then(|temporal| temporal.provenance.source_instant(temporal.source).ok())
        .unwrap_or(u64::MAX)
}

fn budget_refusal(
    policy: &ContextSelectionPolicy,
    used: &SelectedContextCost,
    cost: &SelectedContextCost,
    selected_items: usize,
) -> Result<Option<ContextOmissionReason>, ContextSelectionRefusal> {
    if selected_items >= usize::from(policy.maximum_items) {
        return Ok(Some(ContextOmissionReason::ItemBudget));
    }
    for (total, maximum, reason) in [
        (
            used.bytes.checked_add(cost.bytes),
            policy.maximum_bytes,
            ContextOmissionReason::ByteBudget,
        ),
        (
            used.tokens.checked_add(cost.tokens),
            policy.maximum_tokens,
            ContextOmissionReason::TokenBudget,
        ),
        (
            used.work_units.checked_add(cost.work_units),
            policy.maximum_work_units,
            ContextOmissionReason::WorkBudget,
        ),
    ] {
        let total = total.ok_or(ContextSelectionRefusal::ArithmeticOverflow)?;
        if total > maximum {
            return Ok(Some(reason));
        }
    }
    Ok(None)
}

fn extracted_value_bytes(value: &ExtractedSourceValue) -> Result<u32, ContextSelectionRefusal> {
    let length = match value {
        ExtractedSourceValue::Text(bytes) => bytes.len(),
        ExtractedSourceValue::StructuredItems(items) => sum_lengths(items.iter().map(Vec::len))?,
        ExtractedSourceValue::ResourceMetadata(items) => {
            items.iter().try_fold(0_usize, |total, item| {
                total
                    .checked_add(item.field.len())
                    .and_then(|value| value.checked_add(item.value.len()))
                    .ok_or(ContextSelectionRefusal::ArithmeticOverflow)
            })?
        }
    };
    u32::try_from(length).map_err(|_| ContextSelectionRefusal::ArithmeticOverflow)
}

fn sum_lengths(mut lengths: impl Iterator<Item = usize>) -> Result<usize, ContextSelectionRefusal> {
    lengths.try_fold(0_usize, |total, length| {
        total
            .checked_add(length)
            .ok_or(ContextSelectionRefusal::ArithmeticOverflow)
    })
}

fn validate_identity(identity: &str) -> Result<(), ContextSelectionRefusal> {
    if identity.is_empty() {
        return Err(ContextSelectionRefusal::EmptyIdentity);
    }
    if identity.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(ContextSelectionRefusal::IdentityTooLarge);
    }
    Ok(())
}

pub fn retrieval_paths(item: &SelectedContextItem) -> &[RetrievalContribution] {
    &item.reranked.candidate.contributions
}

pub fn temporal_relation(item: &SelectedContextItem) -> Option<TemporalRelation> {
    item.temporal.as_ref().map(|value| value.context.relation)
}
