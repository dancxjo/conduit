//! Canonical bounded wire values for deterministic hybrid retrieval.

use alloc::{string::String, vec::Vec};

use crate::{
    source_extraction_codec::{
        decode_transport_chunk, encode_transport_chunk, Cursor, SourceExtractionCodecRefusal,
    },
    ExtractedSourceValue, HybridCandidate, HybridRetrievalOutcome, MechanismScore,
    RetrievalContribution, RetrievalMechanism, RetrievalStage, RetrieverIdentity, StageCandidate,
    MAXIMUM_HYBRID_BATCH_BYTES, MAXIMUM_HYBRID_CANDIDATES_PER_STAGE,
    MAXIMUM_HYBRID_OUTPUT_CANDIDATES, MAXIMUM_HYBRID_RETRIEVERS, MAXIMUM_HYBRID_WORK_UNITS,
    MAXIMUM_RAG_IDENTITY_BYTES,
};

const STAGE_VERSION: u8 = 1;
const OUTCOME_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridRetrievalCodecRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    EmptyCandidates,
    CandidateLimitExceeded,
    ContributionLimitExceeded,
    InvalidRank,
    InvalidTemporalIdentity,
    InvalidMechanismScore,
    DuplicateChunk,
    DuplicateRetriever,
    InvalidWork,
    InvalidChunk,
    OutputTooLarge,
    ArithmeticOverflow,
    Malformed,
    UnsupportedVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridRetrievalReceipt {
    pub policy_identity: String,
    pub outcome: HybridRetrievalOutcome<ExtractedSourceValue>,
}

impl RetrievalStage<ExtractedSourceValue> {
    pub fn encode(&self) -> Result<Vec<u8>, HybridRetrievalCodecRefusal> {
        validate_stage(self)?;
        let mut encoded = Vec::new();
        encoded.push(STAGE_VERSION);
        push_identity(&mut encoded, &self.retriever.identity)?;
        encoded.push(mechanism_tag(self.retriever.mechanism));
        encoded.extend_from_slice(&self.work_units.to_le_bytes());
        push_u16(&mut encoded, self.candidates.len())?;
        for candidate in &self.candidates {
            encode_stage_candidate(&mut encoded, candidate)?;
            check_size(&encoded)?;
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, HybridRetrievalCodecRefusal> {
        check_size(encoded)?;
        let mut cursor = Cursor::new(encoded);
        if cursor.u8().map_err(map_source)? != STAGE_VERSION {
            return Err(HybridRetrievalCodecRefusal::UnsupportedVersion);
        }
        let retriever = RetrieverIdentity {
            identity: read_identity(&mut cursor)?,
            mechanism: decode_mechanism(cursor.u8().map_err(map_source)?)?,
        };
        let work_units = cursor.u32().map_err(map_source)?;
        let count = usize::from(cursor.u16().map_err(map_source)?);
        validate_candidate_count(count, MAXIMUM_HYBRID_CANDIDATES_PER_STAGE)?;
        let mut candidates = Vec::with_capacity(count);
        for _ in 0..count {
            candidates.push(decode_stage_candidate(&mut cursor)?);
        }
        if !cursor.finished() {
            return Err(HybridRetrievalCodecRefusal::Malformed);
        }
        let stage = Self {
            retriever,
            candidates,
            work_units,
        };
        validate_stage(&stage)?;
        Ok(stage)
    }
}

impl HybridRetrievalReceipt {
    pub fn encode(&self) -> Result<Vec<u8>, HybridRetrievalCodecRefusal> {
        validate_receipt(self)?;
        let mut encoded = Vec::new();
        encoded.push(OUTCOME_VERSION);
        push_identity(&mut encoded, &self.policy_identity)?;
        match &self.outcome {
            HybridRetrievalOutcome::Candidates(candidates) => {
                encoded.push(0);
                validate_candidate_count(candidates.len(), MAXIMUM_HYBRID_OUTPUT_CANDIDATES)?;
                push_u16(&mut encoded, candidates.len())?;
                for candidate in candidates {
                    encode_hybrid_candidate(&mut encoded, candidate)?;
                    check_size(&encoded)?;
                }
            }
            HybridRetrievalOutcome::NeedEarlierHistory => encoded.push(1),
            HybridRetrievalOutcome::BoundaryUnavailable => encoded.push(2),
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, HybridRetrievalCodecRefusal> {
        check_size(encoded)?;
        let mut cursor = Cursor::new(encoded);
        if cursor.u8().map_err(map_source)? != OUTCOME_VERSION {
            return Err(HybridRetrievalCodecRefusal::UnsupportedVersion);
        }
        let policy_identity = read_identity(&mut cursor)?;
        let outcome = match cursor.u8().map_err(map_source)? {
            0 => {
                let count = usize::from(cursor.u16().map_err(map_source)?);
                validate_candidate_count(count, MAXIMUM_HYBRID_OUTPUT_CANDIDATES)?;
                let mut candidates = Vec::with_capacity(count);
                for _ in 0..count {
                    candidates.push(decode_hybrid_candidate(&mut cursor)?);
                }
                HybridRetrievalOutcome::Candidates(candidates)
            }
            1 => HybridRetrievalOutcome::NeedEarlierHistory,
            2 => HybridRetrievalOutcome::BoundaryUnavailable,
            _ => return Err(HybridRetrievalCodecRefusal::Malformed),
        };
        if !cursor.finished() {
            return Err(HybridRetrievalCodecRefusal::Malformed);
        }
        let receipt = Self {
            policy_identity,
            outcome,
        };
        validate_receipt(&receipt)?;
        Ok(receipt)
    }
}

fn encode_stage_candidate(
    encoded: &mut Vec<u8>,
    candidate: &StageCandidate<ExtractedSourceValue>,
) -> Result<(), HybridRetrievalCodecRefusal> {
    if candidate.rank == 0 {
        return Err(HybridRetrievalCodecRefusal::InvalidRank);
    }
    encode_transport_chunk(encoded, &candidate.chunk).map_err(map_source)?;
    encoded.extend_from_slice(&candidate.rank.to_le_bytes());
    encode_score(encoded, candidate.score);
    push_optional_identity(encoded, candidate.temporal_evidence_identity.as_deref())
}

fn decode_stage_candidate(
    cursor: &mut Cursor<'_>,
) -> Result<StageCandidate<ExtractedSourceValue>, HybridRetrievalCodecRefusal> {
    let chunk = decode_transport_chunk(cursor).map_err(map_source)?;
    let rank = cursor.u16().map_err(map_source)?;
    if rank == 0 {
        return Err(HybridRetrievalCodecRefusal::InvalidRank);
    }
    Ok(StageCandidate {
        chunk,
        rank,
        score: decode_score(cursor)?,
        temporal_evidence_identity: read_optional_identity(cursor)?,
    })
}

fn encode_hybrid_candidate(
    encoded: &mut Vec<u8>,
    candidate: &HybridCandidate<ExtractedSourceValue>,
) -> Result<(), HybridRetrievalCodecRefusal> {
    if candidate.rank == 0 || candidate.contributions.is_empty() {
        return Err(HybridRetrievalCodecRefusal::InvalidRank);
    }
    if candidate.contributions.len() > MAXIMUM_HYBRID_RETRIEVERS {
        return Err(HybridRetrievalCodecRefusal::ContributionLimitExceeded);
    }
    encode_transport_chunk(encoded, &candidate.chunk).map_err(map_source)?;
    encoded.extend_from_slice(&candidate.rank.to_le_bytes());
    encoded.extend_from_slice(&candidate.fusion_score_micros.to_le_bytes());
    push_u16(encoded, candidate.contributions.len())?;
    for contribution in &candidate.contributions {
        push_identity(encoded, &contribution.retriever.identity)?;
        encoded.push(mechanism_tag(contribution.retriever.mechanism));
        if contribution.stage_rank == 0 {
            return Err(HybridRetrievalCodecRefusal::InvalidRank);
        }
        encoded.extend_from_slice(&contribution.stage_rank.to_le_bytes());
        encode_score(encoded, contribution.score);
        push_optional_identity(encoded, contribution.temporal_evidence_identity.as_deref())?;
    }
    Ok(())
}

fn decode_hybrid_candidate(
    cursor: &mut Cursor<'_>,
) -> Result<HybridCandidate<ExtractedSourceValue>, HybridRetrievalCodecRefusal> {
    let chunk = decode_transport_chunk(cursor).map_err(map_source)?;
    let rank = cursor.u16().map_err(map_source)?;
    if rank == 0 {
        return Err(HybridRetrievalCodecRefusal::InvalidRank);
    }
    let fusion_score_micros = cursor.u64().map_err(map_source)?;
    let count = usize::from(cursor.u16().map_err(map_source)?);
    if count == 0 || count > MAXIMUM_HYBRID_RETRIEVERS {
        return Err(HybridRetrievalCodecRefusal::ContributionLimitExceeded);
    }
    let mut contributions = Vec::with_capacity(count);
    for _ in 0..count {
        let retriever = RetrieverIdentity {
            identity: read_identity(cursor)?,
            mechanism: decode_mechanism(cursor.u8().map_err(map_source)?)?,
        };
        let stage_rank = cursor.u16().map_err(map_source)?;
        if stage_rank == 0 {
            return Err(HybridRetrievalCodecRefusal::InvalidRank);
        }
        contributions.push(RetrievalContribution {
            retriever,
            stage_rank,
            score: decode_score(cursor)?,
            temporal_evidence_identity: read_optional_identity(cursor)?,
        });
    }
    Ok(HybridCandidate {
        chunk,
        rank,
        fusion_score_micros,
        contributions,
    })
}

fn encode_score(encoded: &mut Vec<u8>, score: Option<MechanismScore>) {
    match score {
        None => encoded.push(0),
        Some(MechanismScore::SimilarityMicros(value)) => {
            encoded.push(1);
            encoded.extend_from_slice(&value.to_le_bytes());
        }
        Some(MechanismScore::LexicalScore(value)) => {
            encoded.push(2);
            encoded.extend_from_slice(&value.to_le_bytes());
        }
        Some(MechanismScore::MetadataMatch) => encoded.push(3),
        Some(MechanismScore::TemporalBoundary) => encoded.push(4),
        Some(MechanismScore::ExactMatch) => encoded.push(5),
    }
}

fn decode_score(
    cursor: &mut Cursor<'_>,
) -> Result<Option<MechanismScore>, HybridRetrievalCodecRefusal> {
    Ok(match cursor.u8().map_err(map_source)? {
        0 => None,
        1 => Some(MechanismScore::SimilarityMicros(i64::from_le_bytes(
            cursor
                .take(8)
                .map_err(map_source)?
                .try_into()
                .expect("exact score width"),
        ))),
        2 => Some(MechanismScore::LexicalScore(
            cursor.u32().map_err(map_source)?,
        )),
        3 => Some(MechanismScore::MetadataMatch),
        4 => Some(MechanismScore::TemporalBoundary),
        5 => Some(MechanismScore::ExactMatch),
        _ => return Err(HybridRetrievalCodecRefusal::Malformed),
    })
}

const fn mechanism_tag(mechanism: RetrievalMechanism) -> u8 {
    match mechanism {
        RetrievalMechanism::VectorSimilarity => 0,
        RetrievalMechanism::Lexical => 1,
        RetrievalMechanism::Metadata => 2,
        RetrievalMechanism::Temporal => 3,
        RetrievalMechanism::DomainExact => 4,
    }
}

fn decode_mechanism(tag: u8) -> Result<RetrievalMechanism, HybridRetrievalCodecRefusal> {
    match tag {
        0 => Ok(RetrievalMechanism::VectorSimilarity),
        1 => Ok(RetrievalMechanism::Lexical),
        2 => Ok(RetrievalMechanism::Metadata),
        3 => Ok(RetrievalMechanism::Temporal),
        4 => Ok(RetrievalMechanism::DomainExact),
        _ => Err(HybridRetrievalCodecRefusal::Malformed),
    }
}

fn push_optional_identity(
    encoded: &mut Vec<u8>,
    identity: Option<&str>,
) -> Result<(), HybridRetrievalCodecRefusal> {
    match identity {
        None => encoded.push(0),
        Some(identity) => {
            encoded.push(1);
            push_identity(encoded, identity)?;
        }
    }
    Ok(())
}

fn read_optional_identity(
    cursor: &mut Cursor<'_>,
) -> Result<Option<String>, HybridRetrievalCodecRefusal> {
    match cursor.u8().map_err(map_source)? {
        0 => Ok(None),
        1 => Ok(Some(read_identity(cursor)?)),
        _ => Err(HybridRetrievalCodecRefusal::InvalidTemporalIdentity),
    }
}

fn push_identity(encoded: &mut Vec<u8>, identity: &str) -> Result<(), HybridRetrievalCodecRefusal> {
    validate_identity(identity)?;
    let length =
        u16::try_from(identity.len()).map_err(|_| HybridRetrievalCodecRefusal::IdentityTooLarge)?;
    encoded.extend_from_slice(&length.to_le_bytes());
    encoded.extend_from_slice(identity.as_bytes());
    Ok(())
}

fn read_identity(cursor: &mut Cursor<'_>) -> Result<String, HybridRetrievalCodecRefusal> {
    let bytes = cursor.bytes_u16().map_err(map_source)?;
    let identity = core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| HybridRetrievalCodecRefusal::Malformed)?;
    validate_identity(&identity)?;
    Ok(identity)
}

fn validate_identity(identity: &str) -> Result<(), HybridRetrievalCodecRefusal> {
    if identity.is_empty() {
        return Err(HybridRetrievalCodecRefusal::EmptyIdentity);
    }
    if identity.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(HybridRetrievalCodecRefusal::IdentityTooLarge);
    }
    Ok(())
}

fn validate_candidate_count(count: usize, maximum: u16) -> Result<(), HybridRetrievalCodecRefusal> {
    if count == 0 {
        return Err(HybridRetrievalCodecRefusal::EmptyCandidates);
    }
    if count > usize::from(maximum) {
        return Err(HybridRetrievalCodecRefusal::CandidateLimitExceeded);
    }
    Ok(())
}

fn push_u16(encoded: &mut Vec<u8>, value: usize) -> Result<(), HybridRetrievalCodecRefusal> {
    encoded.extend_from_slice(
        &u16::try_from(value)
            .map_err(|_| HybridRetrievalCodecRefusal::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

fn check_size(encoded: &[u8]) -> Result<(), HybridRetrievalCodecRefusal> {
    if encoded.len() > MAXIMUM_HYBRID_BATCH_BYTES as usize {
        Err(HybridRetrievalCodecRefusal::OutputTooLarge)
    } else {
        Ok(())
    }
}

fn validate_stage(
    stage: &RetrievalStage<ExtractedSourceValue>,
) -> Result<(), HybridRetrievalCodecRefusal> {
    validate_identity(&stage.retriever.identity)?;
    validate_candidate_count(stage.candidates.len(), MAXIMUM_HYBRID_CANDIDATES_PER_STAGE)?;
    if stage.work_units == 0 || stage.work_units > MAXIMUM_HYBRID_WORK_UNITS {
        return Err(HybridRetrievalCodecRefusal::InvalidWork);
    }
    for (index, candidate) in stage.candidates.iter().enumerate() {
        if candidate.rank == 0 || usize::from(candidate.rank) > stage.candidates.len() {
            return Err(HybridRetrievalCodecRefusal::InvalidRank);
        }
        if stage.candidates[index + 1..]
            .iter()
            .any(|other| other.chunk.identity == candidate.chunk.identity)
        {
            return Err(HybridRetrievalCodecRefusal::DuplicateChunk);
        }
        validate_contribution_shape(
            stage.retriever.mechanism,
            candidate.score,
            candidate.temporal_evidence_identity.as_deref(),
        )?;
    }
    Ok(())
}

fn validate_receipt(receipt: &HybridRetrievalReceipt) -> Result<(), HybridRetrievalCodecRefusal> {
    validate_identity(&receipt.policy_identity)?;
    let HybridRetrievalOutcome::Candidates(candidates) = &receipt.outcome else {
        return Ok(());
    };
    validate_candidate_count(candidates.len(), MAXIMUM_HYBRID_OUTPUT_CANDIDATES)?;
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.rank != u16::try_from(index + 1).unwrap_or(0)
            || candidate.fusion_score_micros == 0
        {
            return Err(HybridRetrievalCodecRefusal::InvalidRank);
        }
        if candidates[index + 1..]
            .iter()
            .any(|other| other.chunk.identity == candidate.chunk.identity)
        {
            return Err(HybridRetrievalCodecRefusal::DuplicateChunk);
        }
        for (path_index, contribution) in candidate.contributions.iter().enumerate() {
            if candidate.contributions[path_index + 1..]
                .iter()
                .any(|other| other.retriever.identity == contribution.retriever.identity)
            {
                return Err(HybridRetrievalCodecRefusal::DuplicateRetriever);
            }
            validate_contribution_shape(
                contribution.retriever.mechanism,
                contribution.score,
                contribution.temporal_evidence_identity.as_deref(),
            )?;
        }
    }
    Ok(())
}

fn validate_contribution_shape(
    mechanism: RetrievalMechanism,
    score: Option<MechanismScore>,
    temporal_identity: Option<&str>,
) -> Result<(), HybridRetrievalCodecRefusal> {
    let score_matches = matches!(
        (mechanism, score),
        (_, None)
            | (
                RetrievalMechanism::VectorSimilarity,
                Some(MechanismScore::SimilarityMicros(_))
            )
            | (
                RetrievalMechanism::Lexical,
                Some(MechanismScore::LexicalScore(_))
            )
            | (
                RetrievalMechanism::Metadata,
                Some(MechanismScore::MetadataMatch)
            )
            | (
                RetrievalMechanism::Temporal,
                Some(MechanismScore::TemporalBoundary)
            )
            | (
                RetrievalMechanism::DomainExact,
                Some(MechanismScore::ExactMatch)
            )
    );
    if !score_matches {
        return Err(HybridRetrievalCodecRefusal::InvalidMechanismScore);
    }
    match (mechanism, temporal_identity) {
        (RetrievalMechanism::Temporal, Some(identity)) => validate_identity(identity),
        (RetrievalMechanism::Temporal, None) | (_, Some(_)) => {
            Err(HybridRetrievalCodecRefusal::InvalidTemporalIdentity)
        }
        (_, None) => Ok(()),
    }
}

fn map_source(_error: SourceExtractionCodecRefusal) -> HybridRetrievalCodecRefusal {
    HybridRetrievalCodecRefusal::InvalidChunk
}
