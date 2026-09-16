use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{BodyBiographyError, BodyBiographyEvidence, BodyBiographyRecord};
use crate::{BodyId, Wake};

/// One exact, bounded history segment that has left a Body's active window.
///
/// Segments are stored separately from current Body evidence. The active
/// evidence retains only the cumulative boundary and digest of the newest
/// segment, so finite active storage does not impose a finite Body lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBiographyArchiveSegment {
    pub schema: alloc::string::String,
    pub body_id: BodyId,
    pub ordinal: u64,
    pub previous_digest: Option<[u8; 32]>,
    pub records: Vec<BodyBiographyRecord>,
    pub wakes: Vec<Wake>,
    pub digest: [u8; 32],
}

#[derive(Serialize)]
struct ArchivePayload<'a> {
    schema: &'a str,
    body_id: &'a BodyId,
    ordinal: u64,
    previous_digest: Option<[u8; 32]>,
    records: &'a [BodyBiographyRecord],
    wakes: &'a [Wake],
}

impl BodyBiographyArchiveSegment {
    pub(super) fn seal(
        evidence: &BodyBiographyEvidence,
        records: Vec<BodyBiographyRecord>,
        wakes: Vec<Wake>,
    ) -> Result<Self, BodyBiographyError> {
        let previous_digest = evidence
            .compaction
            .as_ref()
            .and_then(|summary| summary.archive_head_digest);
        let ordinal = evidence
            .compaction
            .as_ref()
            .map_or(1, |summary| summary.sealed_segments.saturating_add(1));
        if ordinal == 0 || records.is_empty() || wakes.is_empty() {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let schema = "conduit.body/biography-archive-segment@1";
        let digest = digest(&ArchivePayload {
            schema,
            body_id: &evidence.body_id,
            ordinal,
            previous_digest,
            records: &records,
            wakes: &wakes,
        })?;
        Ok(Self {
            schema: schema.into(),
            body_id: evidence.body_id.clone(),
            ordinal,
            previous_digest,
            records,
            wakes,
            digest,
        })
    }

    pub fn validate(&self) -> Result<(), BodyBiographyError> {
        if self.schema != "conduit.body/biography-archive-segment@1"
            || self.ordinal == 0
            || self.records.is_empty()
            || self.wakes.is_empty()
            || self
                .records
                .windows(2)
                .any(|pair| pair[0].sequence >= pair[1].sequence)
            || self
                .wakes
                .iter()
                .any(|wake| wake.body_id != self.body_id || wake.validate().is_err())
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let expected = digest(&ArchivePayload {
            schema: &self.schema,
            body_id: &self.body_id,
            ordinal: self.ordinal,
            previous_digest: self.previous_digest,
            records: &self.records,
            wakes: &self.wakes,
        })?;
        if expected != self.digest {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }

    pub fn validate_as_head_of(
        &self,
        evidence: &BodyBiographyEvidence,
    ) -> Result<(), BodyBiographyError> {
        self.validate()?;
        let summary = evidence
            .compaction
            .as_ref()
            .ok_or(BodyBiographyError::InvalidEvidence)?;
        if self.body_id != evidence.body_id
            || self.ordinal != summary.sealed_segments
            || Some(self.digest) != summary.archive_head_digest
            || self.records.last().map(|record| record.sequence) != Some(summary.through_sequence)
            || self.records.last().map(|record| &record.sign_id) != Some(&summary.through_sign_id)
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }
}

fn digest(payload: &ArchivePayload<'_>) -> Result<[u8; 32], BodyBiographyError> {
    let bytes = postcard::to_allocvec(payload).map_err(|_| BodyBiographyError::InvalidEvidence)?;
    Ok(Sha256::digest(bytes).into())
}
