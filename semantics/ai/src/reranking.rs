//! Typed finite reranking without promoting scorer output to evidence truth.

use alloc::{string::String, vec::Vec};

use crate::{
    ChunkIdentity, ExtractedSourceValue, HybridCandidate, MAXIMUM_HYBRID_OUTPUT_CANDIDATES,
    MAXIMUM_RAG_IDENTITY_BYTES,
};

pub const MAXIMUM_RERANKING_WORK_UNITS: u32 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankingProofClass {
    DeterministicConformance,
    ModelDerived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerankingStrategy {
    PreserveHybridFusion,
    ObservedScores {
        proof_class: RerankingProofClass,
        scoring_run_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankingPolicy {
    pub identity: String,
    pub strategy: RerankingStrategy,
    pub maximum_candidates: u16,
    pub maximum_work_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RerankObservation {
    pub chunk_identity: ChunkIdentity,
    /// Scorer-local ordering value; never evidence confidence.
    pub score_micros: i64,
    pub work_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankScore {
    HybridFusion(u64),
    ModelDerived(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankedCandidate {
    pub candidate: HybridCandidate<ExtractedSourceValue>,
    pub original_rank: u16,
    pub reranked_rank: u16,
    pub score: RerankScore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankingReceipt {
    pub policy_identity: String,
    pub proof_class: RerankingProofClass,
    pub candidates: Vec<RerankedCandidate>,
    pub work_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankingRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    EmptyCandidates,
    CandidateLimitExceeded,
    InvalidBound,
    InvalidProofClass,
    InvalidCandidate,
    DuplicateCandidate,
    MissingObservation,
    DuplicateObservation,
    UnexpectedObservation,
    ZeroObservationWork,
    WorkBoundExceeded,
    ArithmeticOverflow,
}

impl RerankingPolicy {
    pub fn rerank(
        &self,
        candidates: &[HybridCandidate<ExtractedSourceValue>],
        observations: &[RerankObservation],
    ) -> Result<RerankingReceipt, RerankingRefusal> {
        validate_identity(&self.identity)?;
        validate_candidates(candidates, self.maximum_candidates)?;
        if self.maximum_work_units == 0 || self.maximum_work_units > MAXIMUM_RERANKING_WORK_UNITS {
            return Err(RerankingRefusal::InvalidBound);
        }
        let (proof_class, mut work_units) = match &self.strategy {
            RerankingStrategy::PreserveHybridFusion => {
                if !observations.is_empty() {
                    return Err(RerankingRefusal::UnexpectedObservation);
                }
                (RerankingProofClass::DeterministicConformance, 0)
            }
            RerankingStrategy::ObservedScores {
                proof_class,
                scoring_run_identity,
            } => {
                if *proof_class != RerankingProofClass::ModelDerived {
                    return Err(RerankingRefusal::InvalidProofClass);
                }
                validate_identity(scoring_run_identity)?;
                validate_observations(candidates, observations)?;
                let work = observations.iter().try_fold(0_u32, |total, observation| {
                    total
                        .checked_add(observation.work_units)
                        .ok_or(RerankingRefusal::ArithmeticOverflow)
                })?;
                (*proof_class, work)
            }
        };
        work_units = work_units
            .checked_add(
                u32::try_from(candidates.len())
                    .map_err(|_| RerankingRefusal::ArithmeticOverflow)?,
            )
            .ok_or(RerankingRefusal::ArithmeticOverflow)?;
        if work_units > self.maximum_work_units {
            return Err(RerankingRefusal::WorkBoundExceeded);
        }
        let mut reranked = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let score = match self.strategy {
                RerankingStrategy::PreserveHybridFusion => {
                    RerankScore::HybridFusion(candidate.fusion_score_micros)
                }
                RerankingStrategy::ObservedScores { .. } => RerankScore::ModelDerived(
                    observations
                        .iter()
                        .find(|item| item.chunk_identity == candidate.chunk.identity)
                        .ok_or(RerankingRefusal::MissingObservation)?
                        .score_micros,
                ),
            };
            reranked.push(RerankedCandidate {
                candidate: candidate.clone(),
                original_rank: candidate.rank,
                reranked_rank: 0,
                score,
            });
        }
        reranked.sort_by(compare_reranked);
        for (index, candidate) in reranked.iter_mut().enumerate() {
            candidate.reranked_rank =
                u16::try_from(index + 1).map_err(|_| RerankingRefusal::ArithmeticOverflow)?;
        }
        Ok(RerankingReceipt {
            policy_identity: self.identity.clone(),
            proof_class,
            candidates: reranked,
            work_units,
        })
    }
}

fn validate_candidates(
    candidates: &[HybridCandidate<ExtractedSourceValue>],
    maximum: u16,
) -> Result<(), RerankingRefusal> {
    if candidates.is_empty() {
        return Err(RerankingRefusal::EmptyCandidates);
    }
    if maximum == 0
        || maximum > MAXIMUM_HYBRID_OUTPUT_CANDIDATES
        || candidates.len() > usize::from(maximum)
    {
        return Err(RerankingRefusal::CandidateLimitExceeded);
    }
    for (index, candidate) in candidates.iter().enumerate() {
        candidate
            .chunk
            .validate()
            .map_err(|_| RerankingRefusal::InvalidCandidate)?;
        if candidate.rank == 0
            || candidate.fusion_score_micros == 0
            || candidate.contributions.is_empty()
        {
            return Err(RerankingRefusal::InvalidCandidate);
        }
        if candidates[index + 1..]
            .iter()
            .any(|other| other.chunk.identity == candidate.chunk.identity)
        {
            return Err(RerankingRefusal::DuplicateCandidate);
        }
    }
    Ok(())
}

fn validate_observations(
    candidates: &[HybridCandidate<ExtractedSourceValue>],
    observations: &[RerankObservation],
) -> Result<(), RerankingRefusal> {
    if observations.len() != candidates.len() {
        return Err(RerankingRefusal::MissingObservation);
    }
    for (index, observation) in observations.iter().enumerate() {
        if observation.work_units == 0 {
            return Err(RerankingRefusal::ZeroObservationWork);
        }
        if observations[index + 1..]
            .iter()
            .any(|other| other.chunk_identity == observation.chunk_identity)
        {
            return Err(RerankingRefusal::DuplicateObservation);
        }
        if !candidates
            .iter()
            .any(|candidate| candidate.chunk.identity == observation.chunk_identity)
        {
            return Err(RerankingRefusal::MissingObservation);
        }
    }
    Ok(())
}

fn compare_reranked(left: &RerankedCandidate, right: &RerankedCandidate) -> core::cmp::Ordering {
    match (left.score, right.score) {
        (RerankScore::HybridFusion(left), RerankScore::HybridFusion(right)) => right.cmp(&left),
        (RerankScore::ModelDerived(left), RerankScore::ModelDerived(right)) => right.cmp(&left),
        _ => core::cmp::Ordering::Equal,
    }
    .then_with(|| {
        left.candidate
            .chunk
            .identity
            .cmp(&right.candidate.chunk.identity)
    })
}

fn validate_identity(identity: &str) -> Result<(), RerankingRefusal> {
    if identity.is_empty() {
        return Err(RerankingRefusal::EmptyIdentity);
    }
    if identity.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(RerankingRefusal::IdentityTooLarge);
    }
    Ok(())
}
