//! Bounded metadata from retained compact evidence; original bytes remain authoritative.
use conduit_core::{BorrowedTemporalInstant, TemporalScale};
use serde::Deserialize;
#[path = "episode_retrieval_evidence/decode.rs"]
mod decode;
#[path = "episode_retrieval_evidence/skip.rs"]
mod skip;
#[path = "episode_retrieval_evidence/surrogate_representation.rs"]
mod surrogate_representation;

pub const MAXIMUM_EPISODE_EVIDENCE_BYTES: usize = 4096;
pub const MAXIMUM_EPISODE_EVIDENCE_ITEMS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedEvidenceText<const N: usize> {
    bytes: [u8; N],
    length: usize,
}
impl<const N: usize> FixedEvidenceText<N> {
    pub fn as_str(&self) -> &str {
        // Only a validated str is ever copied into this buffer.
        core::str::from_utf8(&self.bytes[..self.length]).unwrap()
    }
}
impl<const N: usize> Default for FixedEvidenceText<N> {
    fn default() -> Self {
        Self {
            bytes: [0; N],
            length: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeEvidenceOrigin {
    Observation,
    HumanReported,
    DeterministicDerived,
    Remembered,
    ModelDerived,
    Imagined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeObservedInstant {
    pub clock_basis: FixedEvidenceText<256>,
    pub scale: TemporalScale,
    pub ticks: u64,
    pub resolution_ticks: u64,
    pub uncertainty_ticks: u64,
}
impl EpisodeObservedInstant {
    pub fn borrowed(&self) -> BorrowedTemporalInstant<'_> {
        BorrowedTemporalInstant {
            clock_basis: self.clock_basis.as_str(),
            scale: self.scale,
            ticks: self.ticks,
            resolution_ticks: self.resolution_ticks,
            uncertainty_ticks: self.uncertainty_ticks,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EpisodeEvidenceMetadata {
    pub identity: Option<FixedEvidenceText<256>>,
    pub origin: Option<EpisodeEvidenceOrigin>,
    pub observed_at: Option<EpisodeObservedInstant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRetrievalEvidence<'a> {
    pub original_json: &'a [u8],
    pub revision: Option<u64>,
    pub provenance_revision: Option<FixedEvidenceText<256>>,
    items: [EpisodeEvidenceMetadata; MAXIMUM_EPISODE_EVIDENCE_ITEMS],
    length: usize,
}
impl EpisodeRetrievalEvidence<'_> {
    pub fn items(&self) -> &[EpisodeEvidenceMetadata] {
        &self.items[..self.length]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalEvidenceError {
    ByteBound,
    Malformed,
}

pub struct PreparedEpisodeRetrievalEvidence {
    scratch: [u8; MAXIMUM_EPISODE_EVIDENCE_BYTES],
    representation: [u8; MAXIMUM_EPISODE_EVIDENCE_BYTES],
}
impl Default for PreparedEpisodeRetrievalEvidence {
    fn default() -> Self {
        Self::new()
    }
}
impl PreparedEpisodeRetrievalEvidence {
    pub const fn new() -> Self {
        Self {
            scratch: [0; MAXIMUM_EPISODE_EVIDENCE_BYTES],
            representation: [0; MAXIMUM_EPISODE_EVIDENCE_BYTES],
        }
    }
    pub fn parse<'a>(
        &mut self,
        json: &'a [u8],
    ) -> Result<EpisodeRetrievalEvidence<'a>, EpisodeRetrievalEvidenceError> {
        use EpisodeRetrievalEvidenceError::*;
        if json.len() > MAXIMUM_EPISODE_EVIDENCE_BYTES {
            return Err(ByteBound);
        }
        conduit_web::validate_json_text(json).map_err(|_| Malformed)?;
        let adapted_length = surrogate_representation::adapt_validated_surrogate_pairs(
            json,
            &mut self.representation,
        );
        let adapted = &self.representation[..adapted_length];
        serde_json_core::from_slice_escaped::<skip::BoundedDocument>(adapted, &mut self.scratch)
            .map_err(|_| Malformed)?;
        let (parsed, used) =
            serde_json_core::from_slice_escaped::<decode::Root>(adapted, &mut self.scratch)
                .map_err(|_| Malformed)?;
        if adapted[used..].iter().any(|b| !b.is_ascii_whitespace()) {
            return Err(Malformed);
        }
        Ok(EpisodeRetrievalEvidence {
            original_json: json,
            revision: parsed.revision,
            provenance_revision: parsed.provenance_revision,
            items: parsed.items.items,
            length: parsed.items.length,
        })
    }
}
