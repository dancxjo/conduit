//! Finite semantic contracts for retrieval and grounded model results.
//!
//! These types preserve source lineage through retrieval without assigning
//! evidence status to a candidate or grounding confidence to a retrieval
//! score. Extraction, search, ranking, and model providers remain outside the
//! portable contract.

use alloc::{string::String, vec::Vec};
use conduit_core::{BoundedResourceRef, ResourceReferenceRefusal};
use sha2::{Digest, Sha256};

use crate::{ModelResultProvenance, TemporalRetrievalIntent};

pub const MAXIMUM_RETRIEVAL_MODES: usize = 8;
pub const MAXIMUM_RETRIEVAL_CANDIDATES: u16 = 1_024;
pub const MAXIMUM_TRANSFORM_LINEAGE: usize = 16;
pub const MAXIMUM_CONTEXT_ITEMS: usize = 64;
pub const MAXIMUM_CITATIONS: usize = 128;
pub const MAXIMUM_GROUNDED_CLAIMS: usize = 64;
pub const MAXIMUM_GROUNDED_ANSWER_BYTES: usize = 262_144;
pub const MAXIMUM_GROUNDING_LIMITATIONS: usize = 32;
pub const MAXIMUM_RAG_IDENTITY_BYTES: usize = 256;
pub const MAXIMUM_RAG_TEXT_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrievalMode {
    Semantic,
    Exact,
    Metadata,
    Temporal(TemporalRetrievalIntent),
    Boundary(TemporalRetrievalIntent),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalIntent {
    pub identity: String,
    pub modes: Vec<RetrievalMode>,
    pub maximum_candidates: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub resource: BoundedResourceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSpanUnit {
    Bytes,
    Items,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub unit: SourceSpanUnit,
    pub start: u64,
    pub end: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIdentity([u8; 32]);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionLineage {
    pub source: SourceRef,
    pub span: SourceSpan,
    pub extraction_profile: String,
    pub transform_profiles: Vec<String>,
    pub parent_chunk: Option<ChunkIdentity>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<T> {
    pub identity: ChunkIdentity,
    pub lineage: ExtractionLineage,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetrievalScore {
    /// Scheme-specific ordering value. It is neither probability nor evidence.
    pub value_micros: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<T> {
    pub chunk: Chunk<T>,
    pub rank: u16,
    pub score: Option<RetrievalScore>,
    pub retrieval_basis: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSelectionRationale {
    ExactMatch,
    SemanticCandidate,
    MetadataMatch,
    TemporalMatch,
    BoundaryEvidence,
    ConflictPreserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBudgetCost {
    pub bytes: u32,
    pub tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem<T> {
    pub candidate: Candidate<T>,
    pub rationale: ContextSelectionRationale,
    pub budget: ContextBudgetCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTruncationReason {
    ByteBudget,
    TokenBudget,
    ItemBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSelectionOutcome {
    Complete,
    Truncated {
        omitted_candidates: u16,
        reason: ContextTruncationReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSelection<T> {
    pub items: Vec<ContextItem<T>>,
    pub outcome: ContextSelectionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub source: SourceRef,
    pub span: SourceSpan,
    pub chunk_identity: ChunkIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnswerSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundedClaim {
    pub answer_span: AnswerSpan,
    pub citation_indices: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingDisposition {
    Supported,
    InsufficientEvidence,
    ConflictingEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundedResult {
    pub provenance: ModelResultProvenance,
    pub answer_kind: String,
    pub answer: Vec<u8>,
    pub disposition: GroundingDisposition,
    pub claims: Vec<GroundedClaim>,
    pub citations: Vec<Citation>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RagSemanticRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    EmptyIntent,
    TooManyRetrievalModes,
    DuplicateRetrievalMode,
    InvalidTemporalIntent,
    CandidateLimitZero,
    CandidateLimitExceeded,
    InvalidResourceReference,
    EmptySpan,
    SpanOutsideSource,
    MissingItemExtent,
    TooMuchTransformLineage,
    DuplicateTransform,
    ChunkIdentityMismatch,
    RankZero,
    RankExceedsIntent,
    EmptyRetrievalBasis,
    ContextItemLimitExceeded,
    EmptyTruncation,
    EmptyBudget,
    CitationLimitExceeded,
    CitationNotInContext,
    AnswerTooLarge,
    MissingAnswerKind,
    ClaimLimitExceeded,
    InvalidAnswerSpan,
    ClaimWithoutCitation,
    CitationIndexOutOfBounds,
    DuplicateCitationIndex,
    MissingSupportedClaim,
    UnexpectedLimitation,
    MissingLimitation,
    TextBoundExceeded,
}

impl RetrievalIntent {
    pub fn validate(&self) -> Result<(), RagSemanticRefusal> {
        validate_identity(&self.identity)?;
        if self.modes.is_empty() {
            return Err(RagSemanticRefusal::EmptyIntent);
        }
        if self.modes.len() > MAXIMUM_RETRIEVAL_MODES {
            return Err(RagSemanticRefusal::TooManyRetrievalModes);
        }
        for (index, mode) in self.modes.iter().enumerate() {
            if self.modes[index + 1..].contains(mode) {
                return Err(RagSemanticRefusal::DuplicateRetrievalMode);
            }
            if let RetrievalMode::Temporal(intent) | RetrievalMode::Boundary(intent) = mode {
                intent
                    .validate()
                    .map_err(|_| RagSemanticRefusal::InvalidTemporalIntent)?;
            }
        }
        if self.maximum_candidates == 0 {
            return Err(RagSemanticRefusal::CandidateLimitZero);
        }
        if self.maximum_candidates > MAXIMUM_RETRIEVAL_CANDIDATES {
            return Err(RagSemanticRefusal::CandidateLimitExceeded);
        }
        Ok(())
    }
}

impl SourceRef {
    pub fn validate(&self) -> Result<(), RagSemanticRefusal> {
        self.resource.validate().map_err(map_resource_refusal)
    }

    fn canonical_identity(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.resource.identity.digest(),
            self.resource.lifetime.version.digest(),
        )
    }
}

impl SourceSpan {
    pub fn validate_against(&self, source: &SourceRef) -> Result<(), RagSemanticRefusal> {
        source.validate()?;
        if self.start >= self.end {
            return Err(RagSemanticRefusal::EmptySpan);
        }
        let limit = match self.unit {
            SourceSpanUnit::Bytes => source.resource.extent.bytes,
            SourceSpanUnit::Items => source
                .resource
                .extent
                .items
                .ok_or(RagSemanticRefusal::MissingItemExtent)?,
        };
        if self.end > limit {
            return Err(RagSemanticRefusal::SpanOutsideSource);
        }
        Ok(())
    }
}

impl ChunkIdentity {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

impl ExtractionLineage {
    pub fn validate(&self) -> Result<(), RagSemanticRefusal> {
        self.span.validate_against(&self.source)?;
        validate_identity(&self.extraction_profile)?;
        if self.transform_profiles.len() > MAXIMUM_TRANSFORM_LINEAGE {
            return Err(RagSemanticRefusal::TooMuchTransformLineage);
        }
        for (index, transform) in self.transform_profiles.iter().enumerate() {
            validate_identity(transform)?;
            if self.transform_profiles[index + 1..].contains(transform) {
                return Err(RagSemanticRefusal::DuplicateTransform);
            }
        }
        Ok(())
    }

    pub fn identity(&self) -> Result<ChunkIdentity, RagSemanticRefusal> {
        self.validate()?;
        let mut digest = Sha256::new();
        digest.update(b"conduit.ai/rag-chunk-lineage@1\0");
        let (source, version) = self.source.canonical_identity();
        digest.update(source);
        digest.update(version);
        digest.update([match self.span.unit {
            SourceSpanUnit::Bytes => 0,
            SourceSpanUnit::Items => 1,
        }]);
        digest.update(self.span.start.to_le_bytes());
        digest.update(self.span.end.to_le_bytes());
        update_string(&mut digest, &self.extraction_profile);
        digest.update((self.transform_profiles.len() as u16).to_le_bytes());
        for transform in &self.transform_profiles {
            update_string(&mut digest, transform);
        }
        match self.parent_chunk {
            None => digest.update([0]),
            Some(parent) => {
                digest.update([1]);
                digest.update(parent.digest());
            }
        }
        Ok(ChunkIdentity::from_digest(digest.finalize().into()))
    }
}

impl<T> Chunk<T> {
    pub fn new(lineage: ExtractionLineage, value: T) -> Result<Self, RagSemanticRefusal> {
        let identity = lineage.identity()?;
        Ok(Self {
            identity,
            lineage,
            value,
        })
    }

    pub fn validate(&self) -> Result<(), RagSemanticRefusal> {
        if self.identity != self.lineage.identity()? {
            return Err(RagSemanticRefusal::ChunkIdentityMismatch);
        }
        Ok(())
    }
}

impl<T> Candidate<T> {
    pub fn validate_against(&self, intent: &RetrievalIntent) -> Result<(), RagSemanticRefusal> {
        intent.validate()?;
        self.chunk.validate()?;
        if self.rank == 0 {
            return Err(RagSemanticRefusal::RankZero);
        }
        if self.rank > intent.maximum_candidates {
            return Err(RagSemanticRefusal::RankExceedsIntent);
        }
        if self.retrieval_basis.is_empty() {
            return Err(RagSemanticRefusal::EmptyRetrievalBasis);
        }
        validate_text(&self.retrieval_basis)
    }
}

impl<T> ContextItem<T> {
    pub fn validate_against(&self, intent: &RetrievalIntent) -> Result<(), RagSemanticRefusal> {
        self.candidate.validate_against(intent)?;
        if self.budget.bytes == 0 && self.budget.tokens == 0 {
            return Err(RagSemanticRefusal::EmptyBudget);
        }
        Ok(())
    }
}

impl<T> ContextSelection<T> {
    pub fn validate_against(&self, intent: &RetrievalIntent) -> Result<(), RagSemanticRefusal> {
        if self.items.len() > MAXIMUM_CONTEXT_ITEMS {
            return Err(RagSemanticRefusal::ContextItemLimitExceeded);
        }
        for item in &self.items {
            item.validate_against(intent)?;
        }
        if matches!(
            self.outcome,
            ContextSelectionOutcome::Truncated {
                omitted_candidates: 0,
                ..
            }
        ) {
            return Err(RagSemanticRefusal::EmptyTruncation);
        }
        Ok(())
    }
}

impl Citation {
    pub fn validate_against<T>(
        &self,
        context: &ContextSelection<T>,
    ) -> Result<(), RagSemanticRefusal> {
        self.span.validate_against(&self.source)?;
        if context.items.iter().any(|item| {
            item.candidate.chunk.identity == self.chunk_identity
                && item.candidate.chunk.lineage.source == self.source
                && item.candidate.chunk.lineage.span == self.span
        }) {
            Ok(())
        } else {
            Err(RagSemanticRefusal::CitationNotInContext)
        }
    }
}

impl GroundedResult {
    pub fn validate_against<T>(
        &self,
        intent: &RetrievalIntent,
        context: &ContextSelection<T>,
    ) -> Result<(), RagSemanticRefusal> {
        intent.validate()?;
        context.validate_against(intent)?;
        if self.answer_kind.is_empty() {
            return Err(RagSemanticRefusal::MissingAnswerKind);
        }
        validate_identity(&self.answer_kind)?;
        if self.answer.len() > MAXIMUM_GROUNDED_ANSWER_BYTES {
            return Err(RagSemanticRefusal::AnswerTooLarge);
        }
        if self.citations.len() > MAXIMUM_CITATIONS {
            return Err(RagSemanticRefusal::CitationLimitExceeded);
        }
        if self.claims.len() > MAXIMUM_GROUNDED_CLAIMS {
            return Err(RagSemanticRefusal::ClaimLimitExceeded);
        }
        if self.limitations.len() > MAXIMUM_GROUNDING_LIMITATIONS {
            return Err(RagSemanticRefusal::TextBoundExceeded);
        }
        for limitation in &self.limitations {
            validate_text(limitation)?;
        }
        for citation in &self.citations {
            citation.validate_against(context)?;
        }
        for claim in &self.claims {
            if claim.answer_span.start >= claim.answer_span.end
                || claim.answer_span.end as usize > self.answer.len()
            {
                return Err(RagSemanticRefusal::InvalidAnswerSpan);
            }
            if claim.citation_indices.is_empty() {
                return Err(RagSemanticRefusal::ClaimWithoutCitation);
            }
            for (index, citation) in claim.citation_indices.iter().enumerate() {
                if *citation as usize >= self.citations.len() {
                    return Err(RagSemanticRefusal::CitationIndexOutOfBounds);
                }
                if claim.citation_indices[index + 1..].contains(citation) {
                    return Err(RagSemanticRefusal::DuplicateCitationIndex);
                }
            }
        }
        match self.disposition {
            GroundingDisposition::Supported if self.claims.is_empty() => {
                Err(RagSemanticRefusal::MissingSupportedClaim)
            }
            GroundingDisposition::Supported if !self.limitations.is_empty() => {
                Err(RagSemanticRefusal::UnexpectedLimitation)
            }
            GroundingDisposition::InsufficientEvidence
            | GroundingDisposition::ConflictingEvidence
                if self.limitations.is_empty() =>
            {
                Err(RagSemanticRefusal::MissingLimitation)
            }
            _ => Ok(()),
        }
    }
}

fn validate_identity(value: &str) -> Result<(), RagSemanticRefusal> {
    if value.is_empty() {
        return Err(RagSemanticRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_RAG_IDENTITY_BYTES {
        return Err(RagSemanticRefusal::IdentityTooLarge);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), RagSemanticRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_RAG_TEXT_BYTES {
        return Err(RagSemanticRefusal::TextBoundExceeded);
    }
    Ok(())
}

fn update_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u16).to_le_bytes());
    digest.update(value.as_bytes());
}

fn map_resource_refusal(_: ResourceReferenceRefusal) -> RagSemanticRefusal {
    RagSemanticRefusal::InvalidResourceReference
}
