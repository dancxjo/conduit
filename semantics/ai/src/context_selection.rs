//! Typed reranking and finite structured context selection.

use alloc::{string::String, vec::Vec};
use conduit_core::TemporalRelation;
use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    ContextOmission, ContextOmissionReason, ContextOmissions, ContextOrderingPolicy,
    ContextRedundancyPolicy, ContextSelectionDisposition, ContextSelectionPolicy,
    ContextSelectionRefusal, EntityBoundary, ExtractedSourceValue, RerankedCandidate,
    RerankingProofClass, RetrievalContribution, SelectedContextCost, SelectedContextRationale,
    TemporalContext, TemporalProvenance, TemporalSource, TemporalValidity,
    MAXIMUM_HYBRID_OUTPUT_CANDIDATES, MAXIMUM_RAG_IDENTITY_BYTES,
};

mod borrowed;
pub use borrowed::*;
mod prepared;
pub use prepared::*;
#[cfg(test)]
mod borrowed_tests;

pub const MAXIMUM_CONTEXT_SELECTION_WORK_UNITS: u32 = 1_048_576;
pub const MAXIMUM_CONTEXT_BYTES: u32 = 1_048_576;
pub const MAXIMUM_CONTEXT_SELECTION_TOKENS: u32 = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rust's specialization of the authored `ContextTemporalEvidence<TContext>` Type family.
pub struct ContextTemporalEvidence {
    pub evidence_identity: String,
    pub provenance: TemporalProvenance,
    pub source: TemporalSource,
    pub boundary: Option<EntityBoundary>,
    pub context: TemporalContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rust's specialization of the authored `ContextCandidate<T, TContext>` Type family.
pub struct ContextCandidate {
    pub reranked: RerankedCandidate,
    pub reranking_policy_identity: String,
    pub reranking_proof_class: RerankingProofClass,
    pub temporal: Option<ContextTemporalEvidence>,
    /// An exact reviewed grouping fact, never inferred from content at selection time.
    pub redundancy_group: Option<String>,
    pub token_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rust's specialization of the authored `SelectedContextItem<T, TContext>` Type family.
pub struct SelectedContextItem {
    pub reranked: RerankedCandidate,
    pub reranking_policy_identity: String,
    pub reranking_proof_class: RerankingProofClass,
    pub temporal: Option<ContextTemporalEvidence>,
    pub redundancy_group: Option<String>,
    pub rationale: SelectedContextRationale,
    pub budget: SelectedContextCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Rust's specialization of the authored `StructuredContext<T, TContext>` Type family.
pub struct StructuredContext {
    pub policy_identity: String,
    pub token_accounting_profile: String,
    pub redundancy: ContextRedundancyPolicy,
    pub ordering: ContextOrderingPolicy,
    pub items: Vec<SelectedContextItem>,
    pub disposition: ContextSelectionDisposition,
    pub used: SelectedContextCost,
}

impl ContextSelectionPolicy {
    pub fn select(
        &self,
        candidates: &[ContextCandidate],
    ) -> Result<StructuredContext, ContextSelectionRefusal> {
        if candidates.is_empty() {
            return Err(ContextSelectionRefusal::EmptyCandidates);
        }
        if candidates.len() > MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize {
            return Err(ContextSelectionRefusal::CandidateLimitExceeded);
        }
        let views: Vec<_> = candidates
            .iter()
            .map(BorrowedContextCandidate::from_owned)
            .collect();
        let mut selector = PreparedContextSelector::prepare(
            candidates
                .len()
                .clamp(1, MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize),
        )?;
        let mut selected =
            alloc::vec![usize::MAX;candidates.len().min(*self.maximum_items()as usize)];
        let mut omitted = alloc::vec![None;candidates.len()];
        let result = selector.select(self, &views, &mut selected, &mut omitted)?;
        let mut items = Vec::with_capacity(result.selected_len);
        for index in &selected[..result.selected_len] {
            let candidate = &candidates[*index];
            items.push(SelectedContextItem {
                reranked: candidate.reranked.clone(),
                reranking_policy_identity: candidate.reranking_policy_identity.clone(),
                reranking_proof_class: candidate.reranking_proof_class,
                temporal: candidate.temporal.clone(),
                redundancy_group: candidate.redundancy_group.clone(),
                rationale: result.rationale,
                budget: SelectedContextCost {
                    bytes: extracted_value_bytes(&candidate.reranked.candidate.chunk.value)?,
                    tokens: candidate.token_count,
                    work_units: 1,
                },
            });
        }
        let omissions = omitted[..result.omitted_len]
            .iter()
            .flatten()
            .map(|omission| ContextOmission {
                chunk_identity: candidates[omission.candidate_index]
                    .reranked
                    .candidate
                    .chunk
                    .identity,
                reason: omission.reason,
            })
            .collect::<Vec<_>>();
        Ok(StructuredContext {
            policy_identity: self.identity().clone(),
            token_accounting_profile: self.token_accounting_profile().clone(),
            redundancy: *self.redundancy(),
            ordering: *self.ordering(),
            items,
            disposition: if omissions.is_empty() {
                ContextSelectionDisposition::Complete
            } else {
                ContextSelectionDisposition::omitted(
                    ContextOmissions::new(
                        BoundedSequence::try_from_iter(omissions)
                            .expect("validated omission bound"),
                    )
                    .expect("nonempty omissions"),
                )
                .expect("checked omissions")
            },
            used: result.used,
        })
    }
}

fn budget_refusal(
    policy: &ContextSelectionPolicy,
    used: &SelectedContextCost,
    cost: &SelectedContextCost,
    selected_items: usize,
) -> Result<Option<ContextOmissionReason>, ContextSelectionRefusal> {
    if selected_items >= usize::from(*policy.maximum_items()) {
        return Ok(Some(ContextOmissionReason::ItemBudget));
    }
    for (total, maximum, reason) in [
        (
            used.bytes.checked_add(cost.bytes),
            *policy.maximum_bytes(),
            ContextOmissionReason::ByteBudget,
        ),
        (
            used.tokens.checked_add(cost.tokens),
            *policy.maximum_tokens(),
            ContextOmissionReason::TokenBudget,
        ),
        (
            used.work_units.checked_add(cost.work_units),
            *policy.maximum_work_units(),
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
