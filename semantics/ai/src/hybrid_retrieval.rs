//! Finite explicit fusion of independently produced retrieval candidates.

use alloc::{string::String, vec::Vec};

use crate::{
    Chunk, FusionStrategy, MechanismScore, RagSemanticRefusal, RetrievalMechanism,
    RetrieverIdentity, TemporalEvidenceBatch, TemporalEvidenceSelection,
    TemporalEvidenceSelectionRefusal, TemporalRetrievalIntent, MAXIMUM_RAG_IDENTITY_BYTES,
};

pub const MAXIMUM_HYBRID_RETRIEVERS: usize = 8;
pub const MAXIMUM_HYBRID_CANDIDATES_PER_STAGE: u16 = 1_024;
pub const MAXIMUM_HYBRID_OUTPUT_CANDIDATES: u16 = 1_024;
pub const MAXIMUM_HYBRID_WORK_UNITS: u32 = 1_048_576;
const FUSION_SCORE_SCALE: u64 = 1_000_000;

/// A retriever-local observation. These values are retained for inspection
/// and are never compared across mechanisms by the portable fusion policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCandidate<T> {
    pub chunk: Chunk<T>,
    pub rank: u16,
    pub score: Option<MechanismScore>,
    /// Exact temporal-evidence identity, only for a temporal retriever.
    pub temporal_evidence_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalStage<T> {
    pub retriever: RetrieverIdentity,
    pub candidates: Vec<StageCandidate<T>>,
    pub work_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridFusionPolicy {
    pub identity: String,
    /// Adds the fixed fusion scale divided by the rank constant plus stage rank.
    /// Provider scores remain inspection-only and incomparable.
    pub strategy: FusionStrategy,
    pub required_mechanisms: Vec<RetrievalMechanism>,
    pub temporal_hard_filter: Option<TemporalRetrievalIntent>,
    pub maximum_candidates_per_stage: u16,
    pub maximum_output_candidates: u16,
    pub maximum_total_work_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalContribution {
    pub retriever: RetrieverIdentity,
    pub stage_rank: u16,
    pub score: Option<MechanismScore>,
    pub temporal_evidence_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridCandidate<T> {
    pub chunk: Chunk<T>,
    pub rank: u16,
    pub fusion_score_micros: u64,
    pub contributions: Vec<RetrievalContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HybridRetrievalOutcome<T> {
    Candidates(Vec<HybridCandidate<T>>),
    NeedEarlierHistory,
    BoundaryUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridRetrievalRefusal {
    EmptyPolicyIdentity,
    PolicyIdentityTooLarge,
    EmptyStages,
    TooManyStages,
    DuplicateRetriever,
    DuplicateRequiredMechanism,
    MissingRequiredMechanism,
    InvalidPolicyBound,
    InvalidTemporalIntent,
    MissingTemporalEvidence,
    UnexpectedTemporalEvidenceIdentity,
    MissingTemporalEvidenceIdentity,
    TemporalSelection(TemporalEvidenceSelectionRefusal),
    EmptyStage,
    StageCandidateLimitExceeded,
    RankZero,
    RankExceedsStage,
    DuplicateChunkInStage,
    InvalidChunk,
    WorkZero,
    WorkBoundExceeded,
    ArithmeticOverflow,
}

impl HybridFusionPolicy {
    pub fn fuse<T: Clone>(
        &self,
        stages: &[RetrievalStage<T>],
        temporal_evidence: Option<&TemporalEvidenceBatch>,
    ) -> Result<HybridRetrievalOutcome<T>, HybridRetrievalRefusal> {
        self.validate(stages)?;
        let selected_temporal = match &self.temporal_hard_filter {
            None => None,
            Some(intent) => {
                let evidence =
                    temporal_evidence.ok_or(HybridRetrievalRefusal::MissingTemporalEvidence)?;
                match evidence
                    .select(intent)
                    .map_err(HybridRetrievalRefusal::TemporalSelection)?
                {
                    TemporalEvidenceSelection::Selected(payload) => Some(
                        payload
                            .identities()
                            .get()
                            .iter()
                            .map(|identity| identity.get().clone())
                            .collect::<Vec<_>>(),
                    ),
                    TemporalEvidenceSelection::NeedEarlierHistory => {
                        return Ok(HybridRetrievalOutcome::NeedEarlierHistory)
                    }
                    TemporalEvidenceSelection::BoundaryUnavailable => {
                        return Ok(HybridRetrievalOutcome::BoundaryUnavailable)
                    }
                }
            }
        };

        let mut fused: Vec<HybridCandidate<T>> = Vec::new();
        for stage in stages {
            for candidate in &stage.candidates {
                if let Some(existing) = fused
                    .iter_mut()
                    .find(|existing| existing.chunk.identity == candidate.chunk.identity)
                {
                    existing.contributions.push(contribution(stage, candidate));
                    existing.fusion_score_micros = existing
                        .fusion_score_micros
                        .checked_add(self.score(candidate.rank)?)
                        .ok_or(HybridRetrievalRefusal::ArithmeticOverflow)?;
                } else {
                    fused.push(HybridCandidate {
                        chunk: candidate.chunk.clone(),
                        rank: 0,
                        fusion_score_micros: self.score(candidate.rank)?,
                        contributions: alloc::vec![contribution(stage, candidate)],
                    });
                }
            }
        }

        if let Some(selected) = selected_temporal {
            fused.retain(|candidate| {
                candidate.contributions.iter().any(|contribution| {
                    contribution
                        .temporal_evidence_identity
                        .as_ref()
                        .is_some_and(|identity| selected.contains(identity))
                })
            });
            if fused.is_empty() {
                return Ok(HybridRetrievalOutcome::BoundaryUnavailable);
            }
        }

        fused.sort_by(|left, right| {
            right
                .fusion_score_micros
                .cmp(&left.fusion_score_micros)
                .then_with(|| left.chunk.identity.cmp(&right.chunk.identity))
        });
        fused.truncate(usize::from(self.maximum_output_candidates));
        for (index, candidate) in fused.iter_mut().enumerate() {
            candidate.rank =
                u16::try_from(index + 1).map_err(|_| HybridRetrievalRefusal::ArithmeticOverflow)?;
            candidate
                .contributions
                .sort_by(|left, right| left.retriever.identity().cmp(right.retriever.identity()));
        }
        Ok(HybridRetrievalOutcome::Candidates(fused))
    }

    fn validate<T>(&self, stages: &[RetrievalStage<T>]) -> Result<(), HybridRetrievalRefusal> {
        if self.identity.is_empty() {
            return Err(HybridRetrievalRefusal::EmptyPolicyIdentity);
        }
        if self.identity.len() > MAXIMUM_RAG_IDENTITY_BYTES {
            return Err(HybridRetrievalRefusal::PolicyIdentityTooLarge);
        }
        if stages.is_empty() {
            return Err(HybridRetrievalRefusal::EmptyStages);
        }
        if stages.len() > MAXIMUM_HYBRID_RETRIEVERS {
            return Err(HybridRetrievalRefusal::TooManyStages);
        }
        if self.maximum_candidates_per_stage == 0
            || self.maximum_candidates_per_stage > MAXIMUM_HYBRID_CANDIDATES_PER_STAGE
            || self.maximum_output_candidates == 0
            || self.maximum_output_candidates > MAXIMUM_HYBRID_OUTPUT_CANDIDATES
            || self.maximum_total_work_units == 0
            || self.maximum_total_work_units > MAXIMUM_HYBRID_WORK_UNITS
        {
            return Err(HybridRetrievalRefusal::InvalidPolicyBound);
        }
        match &self.strategy {
            FusionStrategy::ReciprocalRank(value) if *value.rank_constant() == 0 => {
                return Err(HybridRetrievalRefusal::InvalidPolicyBound)
            }
            FusionStrategy::ReciprocalRank(_) => {}
        }
        if let Some(intent) = &self.temporal_hard_filter {
            intent
                .validate()
                .map_err(|_| HybridRetrievalRefusal::InvalidTemporalIntent)?;
        }
        for (index, mechanism) in self.required_mechanisms.iter().enumerate() {
            if self.required_mechanisms[index + 1..].contains(mechanism) {
                return Err(HybridRetrievalRefusal::DuplicateRequiredMechanism);
            }
            if !stages
                .iter()
                .any(|stage| *stage.retriever.mechanism() == *mechanism)
            {
                return Err(HybridRetrievalRefusal::MissingRequiredMechanism);
            }
        }

        let mut total_work = 0_u32;
        for (index, stage) in stages.iter().enumerate() {
            if stages[index + 1..]
                .iter()
                .any(|other| other.retriever.identity() == stage.retriever.identity())
            {
                return Err(HybridRetrievalRefusal::DuplicateRetriever);
            }
            if stage.candidates.is_empty() {
                return Err(HybridRetrievalRefusal::EmptyStage);
            }
            if stage.candidates.len() > usize::from(self.maximum_candidates_per_stage) {
                return Err(HybridRetrievalRefusal::StageCandidateLimitExceeded);
            }
            if stage.work_units == 0 {
                return Err(HybridRetrievalRefusal::WorkZero);
            }
            total_work = total_work
                .checked_add(stage.work_units)
                .ok_or(HybridRetrievalRefusal::ArithmeticOverflow)?;
            if total_work > self.maximum_total_work_units {
                return Err(HybridRetrievalRefusal::WorkBoundExceeded);
            }
            for (candidate_index, candidate) in stage.candidates.iter().enumerate() {
                candidate.chunk.validate().map_err(map_chunk_refusal)?;
                if candidate.rank == 0 {
                    return Err(HybridRetrievalRefusal::RankZero);
                }
                if usize::from(candidate.rank) > stage.candidates.len() {
                    return Err(HybridRetrievalRefusal::RankExceedsStage);
                }
                if stage.candidates[candidate_index + 1..]
                    .iter()
                    .any(|other| other.chunk.identity == candidate.chunk.identity)
                {
                    return Err(HybridRetrievalRefusal::DuplicateChunkInStage);
                }
                match (
                    stage.retriever.mechanism(),
                    &candidate.temporal_evidence_identity,
                ) {
                    (RetrievalMechanism::Temporal, None) => {
                        return Err(HybridRetrievalRefusal::MissingTemporalEvidenceIdentity)
                    }
                    (RetrievalMechanism::Temporal, Some(identity)) => {
                        validate_identity(identity)?;
                    }
                    (_, Some(_)) => {
                        return Err(HybridRetrievalRefusal::UnexpectedTemporalEvidenceIdentity)
                    }
                    (_, None) => {}
                }
            }
        }
        Ok(())
    }

    fn score(&self, rank: u16) -> Result<u64, HybridRetrievalRefusal> {
        match &self.strategy {
            FusionStrategy::ReciprocalRank(value) => {
                let denominator = u64::from(*value.rank_constant())
                    .checked_add(u64::from(rank))
                    .ok_or(HybridRetrievalRefusal::ArithmeticOverflow)?;
                Ok(FUSION_SCORE_SCALE / denominator)
            }
        }
    }
}

fn contribution<T>(
    stage: &RetrievalStage<T>,
    candidate: &StageCandidate<T>,
) -> RetrievalContribution {
    RetrievalContribution {
        retriever: stage.retriever.clone(),
        stage_rank: candidate.rank,
        score: candidate.score,
        temporal_evidence_identity: candidate.temporal_evidence_identity.clone(),
    }
}

fn validate_identity(identity: &str) -> Result<(), HybridRetrievalRefusal> {
    if identity.is_empty() {
        return Err(HybridRetrievalRefusal::EmptyPolicyIdentity);
    }
    if identity.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(HybridRetrievalRefusal::PolicyIdentityTooLarge);
    }
    Ok(())
}

fn map_chunk_refusal(_refusal: RagSemanticRefusal) -> HybridRetrievalRefusal {
    HybridRetrievalRefusal::InvalidChunk
}
