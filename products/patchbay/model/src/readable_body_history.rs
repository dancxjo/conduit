//! Bounded human-first projection of one Body's durable Signs biography.
//!
//! “History” is product language for `Body / Signs`, not a fourth semantic
//! Place. The friendly narrative stays subordinate to exact evidence and does
//! not invent clock time or events absent from the validated attachment.

use conduit_body::{BodyBiographyRecord, BodyId};
use conduit_core::SignId;
use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use serde::Serialize;

use crate::{
    PatchbayBodyAttachment, PatchbayBodyEntranceError, MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES,
};

pub const MAX_BODY_HISTORY_TITLE_BYTES: usize = 64;
pub const MAX_BODY_HISTORY_LINEAR_BYTES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadableBodyHistory {
    pub schema: &'static str,
    pub evidence_revision: u64,
    pub body_id: BodyId,
    pub friendly_name: String,
    pub place: PresentationPlace,
    pub aspect: PresentationAspect,
    pub access: BodyHistoryAccess,
    pub entries: Vec<BodyHistoryEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub struct BodyHistoryAccess {
    pub exact_evidence_depth: PresentationDepth,
    pub alternate_manifestation: BodyHistoryManifestation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub enum BodyHistoryManifestation {
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyHistoryEntry {
    pub moment: BodyHistoryMoment,
    pub title: &'static str,
    pub narrative: String,
    pub exact: BodyHistoryExactEvidence,
    pub inspect: BodyHistoryInspectTarget,
    pub linear: String,
}

/// The only temporal claim currently supported by biography evidence.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub enum BodyHistoryMoment {
    EvidenceSequence(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyHistoryExactEvidence {
    pub body_id: BodyId,
    pub record: BodyBiographyRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// A requested exact-evidence destination, not an invented `Follow` relation.
/// A presentation adapter must admit this Sign subject before advertising the
/// corresponding focus operation.
pub struct BodyHistoryInspectTarget {
    pub sign_id: SignId,
    pub subject_identity: String,
    pub place: PresentationPlace,
    pub aspect: PresentationAspect,
    pub depth: PresentationDepth,
}

impl ReadableBodyHistory {
    pub fn from_attachment(
        evidence_revision: u64,
        attachment: &PatchbayBodyAttachment,
    ) -> Result<Self, ReadableBodyHistoryError> {
        let evidence = attachment.evidence();
        if attachment.projection().entries.len() != evidence.records.len() {
            return Err(ReadableBodyHistoryError::ProjectionMismatch);
        }
        let entries = attachment
            .projection()
            .entries
            .iter()
            .zip(&evidence.records)
            .map(
                |(friendly, record)| -> Result<_, ReadableBodyHistoryError> {
                    let linear = linear_record(&evidence.body_id, record);
                    if friendly.sequence != record.sequence
                        || friendly.evidence_sign_id != record.sign_id
                    {
                        return Err(ReadableBodyHistoryError::ProjectionMismatch);
                    }
                    if friendly.heading.len() > MAX_BODY_HISTORY_TITLE_BYTES {
                        return Err(ReadableBodyHistoryError::TitleTooLong);
                    }
                    if friendly.explanation.len() > MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES {
                        return Err(ReadableBodyHistoryError::NarrativeTooLong);
                    }
                    if linear.len() > MAX_BODY_HISTORY_LINEAR_BYTES {
                        return Err(ReadableBodyHistoryError::LinearTooLong);
                    }
                    Ok(BodyHistoryEntry {
                        moment: BodyHistoryMoment::EvidenceSequence(record.sequence),
                        title: friendly.heading,
                        narrative: friendly.explanation.clone(),
                        exact: BodyHistoryExactEvidence {
                            body_id: evidence.body_id.clone(),
                            record: record.clone(),
                        },
                        inspect: BodyHistoryInspectTarget {
                            sign_id: record.sign_id.clone(),
                            subject_identity: format!("sign/{}", record.sign_id.as_str()),
                            place: PresentationPlace::Body,
                            aspect: PresentationAspect::Signs,
                            depth: PresentationDepth::Exact,
                        },
                        linear,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            schema: "conduit.patchbay/readable-body-history@1",
            evidence_revision,
            body_id: evidence.body_id.clone(),
            friendly_name: evidence.friendly_name.clone(),
            place: PresentationPlace::Body,
            aspect: PresentationAspect::Signs,
            access: BodyHistoryAccess {
                exact_evidence_depth: PresentationDepth::Exact,
                alternate_manifestation: BodyHistoryManifestation::Linear,
            },
            entries,
        })
    }
}

fn linear_record(body_id: &BodyId, record: &BodyBiographyRecord) -> String {
    format!(
        "BODY_BIOGRAPHY body={} record={}",
        body_id.as_str(),
        serde_json::to_string(record).expect("validated biography records are serializable")
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadableBodyHistoryError {
    InvalidRevision,
    StaleRevision { current: u64, offered: u64 },
    Entrance(PatchbayBodyEntranceError),
    ProjectionMismatch,
    TitleTooLong,
    NarrativeTooLong,
    LinearTooLong,
}

#[derive(Debug, Default)]
pub struct ReadableBodyHistorySlot {
    last_revision: Option<u64>,
    current: Option<ReadableBodyHistory>,
}

impl ReadableBodyHistorySlot {
    pub fn current(&self) -> Option<&ReadableBodyHistory> {
        self.current.as_ref()
    }

    pub fn replace_attachment(
        &mut self,
        revision: u64,
        attachment: Result<PatchbayBodyAttachment, PatchbayBodyEntranceError>,
    ) -> Result<&ReadableBodyHistory, ReadableBodyHistoryError> {
        if revision == 0 {
            self.current = None;
            return Err(ReadableBodyHistoryError::InvalidRevision);
        }
        if let Some(current) = self.last_revision {
            if revision <= current {
                self.current = None;
                return Err(ReadableBodyHistoryError::StaleRevision {
                    current,
                    offered: revision,
                });
            }
        }
        self.last_revision = Some(revision);
        self.current = None;
        let attachment = attachment.map_err(ReadableBodyHistoryError::Entrance)?;
        self.current = Some(ReadableBodyHistory::from_attachment(revision, &attachment)?);
        Ok(self
            .current
            .as_ref()
            .expect("readable history was installed"))
    }
}
