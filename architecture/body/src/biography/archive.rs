use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{BodyBiographyError, BodyBiographyEvidence, BodyBiographyRecord};
use crate::{BodyId, BodyLifecycleEvent, MembershipEvent, Wake};

pub const MAX_BODY_ARCHIVE_PAGE_SEGMENTS: usize = 8;

/// One bounded, newest-first page loaded on demand from external durable
/// history storage. `next_digest` names the exact predecessor page boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBiographyArchivePage {
    pub segments: Vec<BodyBiographyArchiveSegment>,
    pub next_digest: Option<[u8; 32]>,
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body_events: Vec<BodyLifecycleEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub membership_events: Vec<MembershipEvent>,
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
    body_events: &'a [BodyLifecycleEvent],
    membership_events: &'a [MembershipEvent],
}

#[derive(Serialize)]
struct LegacyArchivePayload<'a> {
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
        body_events: Vec<BodyLifecycleEvent>,
        membership_events: Vec<MembershipEvent>,
    ) -> Result<Self, BodyBiographyError> {
        let previous_digest = evidence
            .compaction
            .as_ref()
            .and_then(|summary| summary.archive_head_digest);
        let ordinal = evidence
            .compaction
            .as_ref()
            .map_or(1, |summary| summary.sealed_segments.saturating_add(1));
        if ordinal == 0
            || records.is_empty()
            || (wakes.is_empty() && body_events.is_empty() && membership_events.is_empty())
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let schema = "conduit.body/biography-archive-segment@2";
        let digest = digest(&ArchivePayload {
            schema,
            body_id: &evidence.body_id,
            ordinal,
            previous_digest,
            records: &records,
            wakes: &wakes,
            body_events: &body_events,
            membership_events: &membership_events,
        })?;
        Ok(Self {
            schema: schema.into(),
            body_id: evidence.body_id.clone(),
            ordinal,
            previous_digest,
            records,
            wakes,
            body_events,
            membership_events,
            digest,
        })
    }

    pub fn validate(&self) -> Result<(), BodyBiographyError> {
        let legacy = self.schema == "conduit.body/biography-archive-segment@1";
        if (!legacy && self.schema != "conduit.body/biography-archive-segment@2")
            || self.ordinal == 0
            || self.records.is_empty()
            || self.records.len() > super::MAX_BODY_BIOGRAPHY_RECORDS
            || self.wakes.len() > super::MAX_BODY_BIOGRAPHY_WAKES
            || self.body_events.len() > crate::MAX_BODY_SIGNS
            || self.membership_events.len() > crate::MAX_MEMBERSHIP_EVENTS
            || (self.wakes.is_empty()
                && self.body_events.is_empty()
                && self.membership_events.is_empty())
            || (legacy && (!self.body_events.is_empty() || !self.membership_events.is_empty()))
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
        let expected = if legacy {
            legacy_digest(&LegacyArchivePayload {
                schema: &self.schema,
                body_id: &self.body_id,
                ordinal: self.ordinal,
                previous_digest: self.previous_digest,
                records: &self.records,
                wakes: &self.wakes,
            })?
        } else {
            digest(&ArchivePayload {
                schema: &self.schema,
                body_id: &self.body_id,
                ordinal: self.ordinal,
                previous_digest: self.previous_digest,
                records: &self.records,
                wakes: &self.wakes,
                body_events: &self.body_events,
                membership_events: &self.membership_events,
            })?
        };
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
            || self
                .records
                .last()
                .is_none_or(|record| record.sequence > summary.through_sequence)
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        if self.records.last().is_some_and(|record| {
            record.sequence == summary.through_sequence && record.sign_id != summary.through_sign_id
        }) {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }

    pub fn load_page(
        evidence: &BodyBiographyEvidence,
        segments: Vec<Self>,
    ) -> Result<BodyBiographyArchivePage, BodyBiographyError> {
        if segments.is_empty() || segments.len() > MAX_BODY_ARCHIVE_PAGE_SEGMENTS {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        segments[0].validate_as_head_of(evidence)?;
        for pair in segments.windows(2) {
            let newer = &pair[0];
            let older = &pair[1];
            older.validate()?;
            if older.body_id != evidence.body_id
                || newer.previous_digest != Some(older.digest)
                || newer.ordinal.checked_sub(1) != Some(older.ordinal)
            {
                return Err(BodyBiographyError::InvalidEvidence);
            }
        }
        let next_digest = segments.last().and_then(|segment| segment.previous_digest);
        Ok(BodyBiographyArchivePage {
            segments,
            next_digest,
        })
    }

    pub fn load_predecessor_page(
        &self,
        segments: Vec<Self>,
    ) -> Result<BodyBiographyArchivePage, BodyBiographyError> {
        self.validate()?;
        let first = segments
            .first()
            .ok_or(BodyBiographyError::InvalidEvidence)?;
        if segments.len() > MAX_BODY_ARCHIVE_PAGE_SEGMENTS
            || self.previous_digest != Some(first.digest)
            || self.ordinal.checked_sub(1) != Some(first.ordinal)
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        first.validate()?;
        for pair in segments.windows(2) {
            let newer = &pair[0];
            let older = &pair[1];
            older.validate()?;
            if older.body_id != self.body_id
                || newer.previous_digest != Some(older.digest)
                || newer.ordinal.checked_sub(1) != Some(older.ordinal)
            {
                return Err(BodyBiographyError::InvalidEvidence);
            }
        }
        let next_digest = segments.last().and_then(|segment| segment.previous_digest);
        Ok(BodyBiographyArchivePage {
            segments,
            next_digest,
        })
    }
}

impl BodyBiographyEvidence {
    /// Seal Body-level workload history after every Wake in the active window
    /// has already reached the archive. Birth remains resident because it binds
    /// Body identity; the checkpoint carries the exact current workload basis.
    pub fn seal_body_workload_history(
        &mut self,
    ) -> Result<Option<BodyBiographyArchiveSegment>, BodyBiographyError> {
        self.validate()?;
        let body_events: Vec<_> = self
            .body
            .events
            .iter()
            .skip(1)
            .filter(|event| !matches!(event, BodyLifecycleEvent::Fulfilled { .. }))
            .cloned()
            .collect();
        if body_events.is_empty() {
            return Ok(None);
        }
        if body_events.iter().any(|event| {
            !matches!(
                event,
                BodyLifecycleEvent::FormAdmitted { .. } | BodyLifecycleEvent::FormRemoved { .. }
            )
        }) {
            return Ok(None);
        }
        let sign_ids: Vec<_> = body_events
            .iter()
            .map(|event| event.sign_id().clone())
            .collect();
        let records: Vec<_> = self
            .records
            .iter()
            .filter(|record| sign_ids.contains(&record.sign_id))
            .cloned()
            .collect();
        if records.len() != body_events.len() {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let segment = BodyBiographyArchiveSegment::seal(
            self,
            records.clone(),
            Vec::new(),
            body_events,
            Vec::new(),
        )?;
        let mut candidate = self.clone();
        candidate.body.history_checkpoint = Some(crate::BodyHistoryCheckpoint {
            workset: candidate.body.workset.clone(),
            workload_revision: candidate.body.workload_revision,
        });
        let terminal = candidate
            .body
            .events
            .iter()
            .find(|event| matches!(event, BodyLifecycleEvent::Fulfilled { .. }))
            .cloned();
        candidate.body.events.truncate(1);
        candidate.body.sign_ids.truncate(1);
        if let Some(terminal) = terminal {
            candidate.body.sign_ids.push(terminal.sign_id().clone());
            candidate.body.events.push(terminal);
        }
        candidate
            .records
            .retain(|record| !sign_ids.contains(&record.sign_id));
        candidate.advance_compaction(&segment, &records, 0, None)?;
        candidate.validate()?;
        *self = candidate;
        Ok(Some(segment))
    }

    /// Seal the retained membership event suffix behind a checkpoint of exact
    /// current Parts. The returned segment must commit with this active state.
    pub fn seal_membership_history(
        &mut self,
    ) -> Result<Option<BodyBiographyArchiveSegment>, BodyBiographyError> {
        self.validate()?;
        if self.membership.events.is_empty() {
            return Ok(None);
        }
        let membership_events = self.membership.events.clone();
        let change_ids: Vec<_> = membership_events
            .iter()
            .map(|event| event.change_id.clone())
            .collect();
        let records: Vec<_> = self
            .records
            .iter()
            .filter(|record| match &record.kind {
                super::BodyBiographyRecordKind::PartAdmitted { change_id, .. }
                | super::BodyBiographyRecordKind::HostJoined { change_id, .. }
                | super::BodyBiographyRecordKind::HostLeft { change_id, .. }
                | super::BodyBiographyRecordKind::PartRevoked { change_id, .. } => {
                    change_ids.contains(change_id)
                }
                _ => false,
            })
            .cloned()
            .collect();
        if records.len() != membership_events.len() {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let segment = BodyBiographyArchiveSegment::seal(
            self,
            records.clone(),
            Vec::new(),
            Vec::new(),
            membership_events,
        )?;
        let mut candidate = self.clone();
        candidate
            .membership
            .checkpoint_events()
            .map_err(|_| BodyBiographyError::InvalidEvidence)?;
        candidate.records.retain(|record| match &record.kind {
            super::BodyBiographyRecordKind::PartAdmitted { change_id, .. }
            | super::BodyBiographyRecordKind::HostJoined { change_id, .. }
            | super::BodyBiographyRecordKind::HostLeft { change_id, .. }
            | super::BodyBiographyRecordKind::PartRevoked { change_id, .. } => {
                !change_ids.contains(change_id)
            }
            _ => true,
        });
        candidate.advance_compaction(&segment, &records, 0, None)?;
        candidate.validate()?;
        *self = candidate;
        Ok(Some(segment))
    }

    fn advance_compaction(
        &mut self,
        segment: &BodyBiographyArchiveSegment,
        records: &[BodyBiographyRecord],
        wakes: u64,
        first_wake_id: Option<crate::WakeId>,
    ) -> Result<(), BodyBiographyError> {
        let last = records.last().ok_or(BodyBiographyError::InvalidEvidence)?;
        let prior = self.compaction.as_ref();
        let (through_sequence, through_sign_id) =
            if prior.is_some_and(|summary| summary.through_sequence >= last.sequence) {
                let summary = prior.expect("checked above");
                (summary.through_sequence, summary.through_sign_id.clone())
            } else {
                (last.sequence, last.sign_id.clone())
            };
        self.compaction = Some(super::BodyBiographyCompaction {
            wakes: prior
                .map_or(0, |summary| summary.wakes)
                .checked_add(wakes)
                .ok_or(BodyBiographyError::CapacityExhausted)?,
            records: prior
                .map_or(0, |summary| summary.records)
                .checked_add(records.len() as u64)
                .ok_or(BodyBiographyError::CapacityExhausted)?,
            through_sequence,
            through_sign_id,
            first_wake_id: prior
                .and_then(|summary| summary.first_wake_id.clone())
                .or(first_wake_id),
            sealed_segments: segment.ordinal,
            archive_head_digest: Some(segment.digest),
        });
        Ok(())
    }
}

fn digest(payload: &ArchivePayload<'_>) -> Result<[u8; 32], BodyBiographyError> {
    let bytes = postcard::to_allocvec(payload).map_err(|_| BodyBiographyError::InvalidEvidence)?;
    Ok(Sha256::digest(bytes).into())
}

fn legacy_digest(payload: &LegacyArchivePayload<'_>) -> Result<[u8; 32], BodyBiographyError> {
    let bytes = postcard::to_allocvec(payload).map_err(|_| BodyBiographyError::InvalidEvidence)?;
    Ok(Sha256::digest(bytes).into())
}
