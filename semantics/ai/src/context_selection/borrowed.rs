//! Borrowed candidate custody. No source dereference or allocation is performed.
use super::*;
use conduit_core::{BorrowedTemporalInstant, BoundedResourceRef, TemporalScale};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
pub enum BorrowedContextSource<'a> {
    Owned(&'a BoundedResourceRef),
    Encoded(&'a [u8]),
}
#[derive(Clone, Copy)]
pub struct BorrowedContextLineage<'a> {
    pub source: BorrowedContextSource<'a>,
    pub span_unit: crate::SourceSpanUnit,
    pub span_start: u64,
    pub span_end: u64,
    pub extraction_profile: &'a str,
    pub transform_profiles: &'a [&'a str],
    pub parent_chunk: Option<crate::ChunkIdentity>,
}
#[derive(Clone, Copy)]
pub enum BorrowedContextValue<'a> {
    Text(&'a [u8]),
    StructuredItems(&'a [&'a [u8]]),
    ResourceMetadata(&'a [(&'a str, &'a str)]),
}
#[derive(Clone, Copy)]
pub enum BorrowedContextChunk<'a> {
    Owned(&'a crate::Chunk<ExtractedSourceValue>),
    Explicit {
        identity: crate::ChunkIdentity,
        lineage: BorrowedContextLineage<'a>,
        value: BorrowedContextValue<'a>,
    },
}
impl BorrowedContextChunk<'_> {
    pub fn identity(&self) -> crate::ChunkIdentity {
        match self {
            Self::Owned(chunk) => chunk.identity,
            Self::Explicit { identity, .. } => *identity,
        }
    }
    pub fn validate(&self) -> Result<(), ContextSelectionRefusal> {
        match self {
            Self::Explicit {
                identity, lineage, ..
            } => validate_lineage(*identity, *lineage),
            Self::Owned(chunk) => {
                let owned = &chunk.lineage;
                let profiles = owned.transform_profiles.get();
                if profiles.len() > crate::MAXIMUM_TRANSFORM_LINEAGE {
                    return Err(ContextSelectionRefusal::InvalidCandidate);
                }
                let mut transforms = [""; crate::MAXIMUM_TRANSFORM_LINEAGE];
                for (slot, profile) in transforms.iter_mut().zip(profiles.iter()) {
                    *slot = profile.get();
                }
                validate_lineage(
                    chunk.identity,
                    BorrowedContextLineage {
                        source: BorrowedContextSource::Owned(&owned.source.resource),
                        span_unit: owned.span.unit(),
                        span_start: owned.span.start(),
                        span_end: owned.span.end(),
                        extraction_profile: owned.extraction_profile.get(),
                        transform_profiles: &transforms[..profiles.len()],
                        parent_chunk: owned.parent_chunk,
                    },
                )
            }
        }
    }
    pub fn byte_cost(&self) -> Result<u32, ContextSelectionRefusal> {
        let length = match self {
            Self::Owned(chunk) => return extracted_value_bytes(&chunk.value),
            Self::Explicit { value, .. } => match value {
                BorrowedContextValue::Text(bytes) => bytes.len(),
                BorrowedContextValue::StructuredItems(items) => {
                    sum_lengths(items.iter().map(|item| item.len()))?
                }
                BorrowedContextValue::ResourceMetadata(items) => {
                    items.iter().try_fold(0usize, |total, (key, value)| {
                        total
                            .checked_add(key.len())
                            .and_then(|n| n.checked_add(value.len()))
                            .ok_or(ContextSelectionRefusal::ArithmeticOverflow)
                    })?
                }
            },
        };
        u32::try_from(length).map_err(|_| ContextSelectionRefusal::ArithmeticOverflow)
    }
}
fn validate_lineage(
    identity: crate::ChunkIdentity,
    lineage: BorrowedContextLineage<'_>,
) -> Result<(), ContextSelectionRefusal> {
    let (source, version, extent) = match lineage.source {
        BorrowedContextSource::Owned(resource) => {
            resource
                .validate()
                .map_err(|_| ContextSelectionRefusal::InvalidCandidate)?;
            (
                resource.identity.digest(),
                resource.lifetime.version.digest(),
                resource.extent,
            )
        }
        BorrowedContextSource::Encoded(encoded) => {
            let resource = BoundedResourceRef::validate_encoded(encoded)
                .map_err(|_| ContextSelectionRefusal::InvalidCandidate)?;
            (
                resource.identity.digest(),
                resource.version.digest(),
                resource.extent,
            )
        }
    };
    let limit = match lineage.span_unit {
        crate::SourceSpanUnit::Bytes => extent.bytes,
        crate::SourceSpanUnit::Items => extent
            .items
            .ok_or(ContextSelectionRefusal::InvalidCandidate)?,
    };
    if lineage.span_start >= lineage.span_end
        || lineage.span_end > limit
        || lineage.transform_profiles.len() > crate::MAXIMUM_TRANSFORM_LINEAGE
    {
        return Err(ContextSelectionRefusal::InvalidCandidate);
    }
    validate_identity(lineage.extraction_profile)
        .map_err(|_| ContextSelectionRefusal::InvalidCandidate)?;
    for (index, profile) in lineage.transform_profiles.iter().enumerate() {
        validate_identity(profile).map_err(|_| ContextSelectionRefusal::InvalidCandidate)?;
        if lineage.transform_profiles[index + 1..].contains(profile) {
            return Err(ContextSelectionRefusal::InvalidCandidate);
        }
    }
    let mut digest = Sha256::new();
    digest.update(b"conduit.ai/rag-chunk-lineage@1\0");
    digest.update(source);
    digest.update(version);
    digest.update([if lineage.span_unit == crate::SourceSpanUnit::Bytes {
        0
    } else {
        1
    }]);
    digest.update(lineage.span_start.to_le_bytes());
    digest.update(lineage.span_end.to_le_bytes());
    hash_string(&mut digest, lineage.extraction_profile);
    digest.update((lineage.transform_profiles.len() as u16).to_le_bytes());
    for profile in lineage.transform_profiles {
        hash_string(&mut digest, profile);
    }
    match lineage.parent_chunk {
        None => digest.update([0]),
        Some(parent) => {
            digest.update([1]);
            digest.update(parent.digest());
        }
    }
    let expected: [u8; 32] = digest.finalize().into();
    if identity.digest() != expected {
        return Err(ContextSelectionRefusal::InvalidCandidate);
    }
    Ok(())
}

fn hash_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u16).to_le_bytes());
    digest.update(value.as_bytes());
}
#[derive(Clone, Copy)]
pub enum BorrowedContextContributions<'a> {
    Owned(&'a [RetrievalContribution]),
    Explicit(&'a [Option<&'a str>]),
}
impl BorrowedContextContributions<'_> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Owned(paths) => paths.len(),
            Self::Explicit(paths) => paths.len(),
        }
    }
    pub(super) fn any_temporal(self) -> bool {
        match self {
            Self::Owned(paths) => paths.iter().any(|p| p.temporal_evidence_identity.is_some()),
            Self::Explicit(paths) => paths.iter().any(Option::is_some),
        }
    }
    pub(super) fn contains(self, id: &str) -> bool {
        match self {
            Self::Owned(paths) => paths
                .iter()
                .any(|p| p.temporal_evidence_identity.as_deref() == Some(id)),
            Self::Explicit(paths) => paths.contains(&Some(id)),
        }
    }
}
#[derive(Clone, Copy)]
pub struct BorrowedContextTemporal<'a> {
    pub evidence_identity: &'a str,
    pub source: TemporalSource,
    pub source_at: Option<u64>,
    pub valid_from: Option<u64>,
    pub valid_until: Option<u64>,
    pub retrieved_at: u64,
    pub reference_at: u64,
    pub clock_basis: &'a str,
    pub uncertainty_millis: Option<u64>,
    pub context_source: TemporalSource,
    pub context_relation: TemporalRelation,
    pub context_validity: TemporalValidity,
}
impl<'a> BorrowedContextTemporal<'a> {
    pub fn from_owned(value: &'a ContextTemporalEvidence) -> Self {
        let p = &value.provenance;
        Self {
            evidence_identity: &value.evidence_identity,
            source: value.source,
            source_at: p.source_instant(value.source).ok(),
            valid_from: p.valid_from,
            valid_until: p.valid_until,
            retrieved_at: p.retrieved_at,
            reference_at: p.reference_at,
            clock_basis: match &p.clock_basis {
                crate::ClockBasis::UnixEpochMilliseconds => conduit_core::UNIX_UTC_CLOCK_BASIS,
                crate::ClockBasis::MonotonicMilliseconds(value) => value.identity().get(),
            },
            uncertainty_millis: p.uncertainty_millis,
            context_source: value.context.source,
            context_relation: value.context.relation,
            context_validity: value.context.validity,
        }
    }
    pub(super) fn validate(
        self,
        contributions: BorrowedContextContributions<'_>,
    ) -> Result<(), ContextSelectionRefusal> {
        validate_identity(self.evidence_identity)?;
        if self.clock_basis.is_empty()
            || self.clock_basis.len() > 128
            || self.source == TemporalSource::ValidFrom && self.source_at != self.valid_from
            || self.source == TemporalSource::ValidUntil && self.source_at != self.valid_until
            || self.source == TemporalSource::Retrieved && self.source_at != Some(self.retrieved_at)
        {
            return Err(ContextSelectionRefusal::InvalidTemporalEvidence);
        }

        if self
            .valid_from
            .zip(self.valid_until)
            .is_some_and(|(start, end)| start > end)
            || self.retrieved_at > self.reference_at
        {
            return Err(ContextSelectionRefusal::InvalidTemporalEvidence);
        }
        let observed = BorrowedTemporalInstant {
            ticks: self
                .source_at
                .ok_or(ContextSelectionRefusal::InvalidTemporalEvidence)?,
            scale: TemporalScale::Milliseconds,
            clock_basis: self.clock_basis,
            resolution_ticks: 1,
            uncertainty_ticks: self.uncertainty_millis.unwrap_or(0),
        };
        let reference = BorrowedTemporalInstant {
            ticks: self.reference_at,
            uncertainty_ticks: 0,
            ..observed
        };
        let relation = observed
            .relation_to(&reference)
            .map_err(|_| ContextSelectionRefusal::InvalidTemporalEvidence)?;
        if relation != self.context_relation
            || self.source != self.context_source
            || self.context_validity == TemporalValidity::UnknownWhetherCurrent
                && self.valid_from.is_some()
            || !contributions.contains(self.evidence_identity)
        {
            return Err(ContextSelectionRefusal::TemporalEvidenceNotContributed);
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct BorrowedContextCandidate<'a> {
    pub chunk: BorrowedContextChunk<'a>,
    pub reranked_rank: u16,
    pub original_rank: u16,
    pub contributions: BorrowedContextContributions<'a>,
    pub reranking_policy_identity: &'a str,
    pub temporal: Option<BorrowedContextTemporal<'a>>,
    pub redundancy_group: Option<&'a str>,
    pub token_count: u32,
}
impl<'a> BorrowedContextCandidate<'a> {
    pub fn from_owned(value: &'a ContextCandidate) -> Self {
        Self {
            chunk: BorrowedContextChunk::Owned(&value.reranked.candidate.chunk),
            reranked_rank: value.reranked.reranked_rank,
            original_rank: value.reranked.original_rank,
            contributions: BorrowedContextContributions::Owned(
                &value.reranked.candidate.contributions,
            ),
            reranking_policy_identity: &value.reranking_policy_identity,
            temporal: value
                .temporal
                .as_ref()
                .map(BorrowedContextTemporal::from_owned),
            redundancy_group: value.redundancy_group.as_deref(),
            token_count: value.token_count,
        }
    }
}
