//! Deterministic bounded exact-search oracle for vector conformance.

use alloc::{collections::BTreeSet, string::String, vec::Vec};
use conduit_core::ResourceBinding;
use serde::{Deserialize, Serialize};

use crate::{
    canonical_hit_order, EntityBoundary, MetadataFilter, SimilarityHit, SimilarityQuery,
    TemporalEvidenceBatch, TemporalEvidenceCandidate, TemporalEvidenceSelection,
    TemporalEvidenceSelectionRefusal, TemporalReference, TemporalSource, TemporalValidity,
    TransitionDirection, VectorIndexHandle, VectorIndexQueryAdmission, VectorIndexResourceRefusal,
    VectorIndexState, VectorRecord, VectorRefusal, MAXIMUM_VECTOR_INDEX_MEMBERS,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExactVectorSearchCandidate<T> {
    pub record: VectorRecord<T>,
    pub temporal_source: TemporalSource,
    pub boundary: Option<EntityBoundary>,
    pub transition: Option<TransitionDirection>,
    pub validity: TemporalValidity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorSearchProofClass {
    DeterministicExactOracle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExactVectorSearchResult<T> {
    pub proof_class: VectorSearchProofClass,
    pub index_generation: u64,
    pub admitted_work_units: u32,
    pub candidate_count: u32,
    pub hits: Vec<SimilarityHit<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactVectorSearchRefusal {
    Vector(VectorRefusal),
    Resource(VectorIndexResourceRefusal),
    Temporal(TemporalEvidenceSelectionRefusal),
    TooManyCandidates,
    IndexMembershipMismatch,
    WorkAccountingOverflow,
    TemporalProvenanceRequired,
    EarlierHistoryRequired,
    TemporalBoundaryUnavailable,
}

/// Search a caller-supplied finite fixture exactly after ordinary V1 query admission.
///
/// One work unit is one candidate dimension. The entire input slice is charged before
/// validation, filtering, temporal selection, or scoring, so selective queries cannot
/// hide an unadmitted scan.
#[allow(clippy::too_many_arguments)]
pub fn exact_vector_search<T: Clone>(
    state: &VectorIndexState,
    handle: &VectorIndexHandle,
    query: &SimilarityQuery,
    candidates: &[ExactVectorSearchCandidate<T>],
    admission: VectorIndexQueryAdmission,
    binding: &ResourceBinding,
    earliest_history_complete: bool,
) -> Result<ExactVectorSearchResult<T>, ExactVectorSearchRefusal> {
    query.validate().map_err(ExactVectorSearchRefusal::Vector)?;
    state
        .contract
        .embedding_profile
        .compatibility(&query.embedding.profile, query.metric)
        .map_err(ExactVectorSearchRefusal::Vector)?;
    state
        .admit_query(handle, admission, binding)
        .map_err(ExactVectorSearchRefusal::Resource)?;
    if candidates.len() > MAXIMUM_VECTOR_INDEX_MEMBERS as usize {
        return Err(ExactVectorSearchRefusal::TooManyCandidates);
    }
    let candidate_count =
        u32::try_from(candidates.len()).map_err(|_| ExactVectorSearchRefusal::TooManyCandidates)?;
    let required_work = candidate_count
        .checked_mul(query.embedding.profile.dimensions)
        .ok_or(ExactVectorSearchRefusal::WorkAccountingOverflow)?;
    if required_work > admission.work_units {
        return Err(ExactVectorSearchRefusal::Resource(
            VectorIndexResourceRefusal::QueryWorkLimitExceeded,
        ));
    }
    if query.top_k > admission.maximum_results {
        return Err(ExactVectorSearchRefusal::Resource(
            VectorIndexResourceRefusal::ResultLimitExceeded,
        ));
    }

    for candidate in candidates {
        candidate
            .record
            .validate()
            .map_err(ExactVectorSearchRefusal::Vector)?;
        query
            .embedding
            .profile
            .compatibility(&candidate.record.embedding.profile, query.metric)
            .map_err(ExactVectorSearchRefusal::Vector)?;
    }
    let candidate_sources: BTreeSet<_> = candidates
        .iter()
        .map(|candidate| candidate.record.source_identity.as_str())
        .collect();
    let member_sources: BTreeSet<_> = state
        .members()
        .iter()
        .map(|member| member.source_identity.as_str())
        .collect();
    if candidate_sources.len() != candidates.len()
        || candidate_sources.len() != state.members().len()
        || candidate_sources != member_sources
    {
        return Err(ExactVectorSearchRefusal::IndexMembershipMismatch);
    }

    let mut eligible: Vec<_> = candidates
        .iter()
        .filter(|candidate| metadata_matches(&candidate.record, &query.filters))
        .collect();
    if let Some(intent) = &query.temporal_intent {
        eligible = temporal_matches(eligible, intent, earliest_history_complete)?;
    }

    let mut hits = Vec::with_capacity(core::cmp::min(eligible.len(), query.top_k as usize));
    for candidate in eligible {
        let score = query
            .score(&candidate.record.embedding)
            .map_err(ExactVectorSearchRefusal::Vector)?;
        if !query
            .admits_score(score)
            .map_err(ExactVectorSearchRefusal::Vector)?
        {
            continue;
        }
        hits.push(SimilarityHit {
            value: candidate.record.value.clone(),
            score,
            rank: 1,
            index_generation: state.contract.generation,
            source_identity: candidate.record.source_identity.clone(),
            resource_identity: candidate.record.resource_identity.clone(),
            temporal_provenance: candidate.record.temporal_provenance.clone(),
        });
    }
    hits.sort_by(canonical_hit_order);
    hits.truncate(query.top_k as usize);
    for (index, hit) in hits.iter_mut().enumerate() {
        hit.rank = u32::try_from(index + 1).expect("top-k is bounded by u32");
    }

    Ok(ExactVectorSearchResult {
        proof_class: VectorSearchProofClass::DeterministicExactOracle,
        index_generation: state.contract.generation,
        admitted_work_units: admission.work_units,
        candidate_count,
        hits,
    })
}

fn metadata_matches<T>(record: &VectorRecord<T>, filters: &[MetadataFilter]) -> bool {
    filters.iter().all(|filter| match filter {
        MetadataFilter::Equal { key, value } => record
            .metadata
            .iter()
            .any(|member| member.key == *key && member.value == *value),
        MetadataFilter::Present { key } => record.metadata.iter().any(|member| member.key == *key),
    })
}

fn temporal_matches<'a, T>(
    candidates: Vec<&'a ExactVectorSearchCandidate<T>>,
    intent: &crate::TemporalRetrievalIntent,
    earliest_history_complete: bool,
) -> Result<Vec<&'a ExactVectorSearchCandidate<T>>, ExactVectorSearchRefusal> {
    if candidates.is_empty() {
        return Ok(candidates);
    }
    let first = candidates[0]
        .record
        .temporal_provenance
        .as_ref()
        .ok_or(ExactVectorSearchRefusal::TemporalProvenanceRequired)?;
    let batch = TemporalEvidenceBatch {
        reference: TemporalReference {
            reference_at: first.reference_at,
            clock_basis: first.clock_basis.clone(),
        },
        candidates: candidates
            .iter()
            .map(|candidate| {
                let provenance = candidate
                    .record
                    .temporal_provenance
                    .clone()
                    .ok_or(ExactVectorSearchRefusal::TemporalProvenanceRequired)?;
                Ok(TemporalEvidenceCandidate {
                    identity: candidate.record.source_identity.clone(),
                    provenance,
                    source: candidate.temporal_source,
                    boundary: candidate.boundary,
                    transition: candidate.transition,
                    validity: candidate.validity,
                })
            })
            .collect::<Result<Vec<_>, ExactVectorSearchRefusal>>()?,
        earliest_history_complete,
    };
    let selected = batch
        .select(intent)
        .map_err(ExactVectorSearchRefusal::Temporal)?;
    let identities: BTreeSet<String> = match selected {
        TemporalEvidenceSelection::Selected { identities } => identities.into_iter().collect(),
        TemporalEvidenceSelection::NeedEarlierHistory => {
            return Err(ExactVectorSearchRefusal::EarlierHistoryRequired)
        }
        TemporalEvidenceSelection::BoundaryUnavailable => {
            return Err(ExactVectorSearchRefusal::TemporalBoundaryUnavailable)
        }
    };
    Ok(candidates
        .into_iter()
        .filter(|candidate| identities.contains(&candidate.record.source_identity))
        .collect())
}
