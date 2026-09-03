//! Browser transport for the shared current-Body and readable-biography models.
//!
//! This is a finite renderer DTO. It is built only from validated Patchbay
//! models and never decodes Crèche evidence or becomes a second Body store.

use conduit_body::{BodyBiographyRecord, BodyId, PartId, WakeId, MAX_BODY_BIOGRAPHY_RECORDS};
use conduit_core::{
    BootId, CheckedFormId, HostId, ImplementationId, OfferGeneration, PlanId, SignId,
    SourceDocumentId,
};
use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use patchbay_model::{
    BodyHistoryManifestation, BodyHistoryMoment, CurrentBodyFrame, CurrentBodyLifecycle,
    CurrentBodyLifecycleAction, CurrentBodyPatchbayReader, CurrentBodyPhysicalHostSummary,
    ReadableBodyHistory, MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES, MAX_BODY_HISTORY_LINEAR_BYTES,
    MAX_BODY_HISTORY_TITLE_BYTES,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserBodyWorkbench {
    pub schema: String,
    pub evidence_revision: u64,
    pub current: BrowserCurrentBody,
    pub history: BrowserBodyHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserCurrentBody {
    pub body_id: BodyId,
    pub friendly_name: String,
    pub program_label: String,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub lifecycle: BrowserBodyLifecycle,
    pub admitted_parts: usize,
    pub current_hosts: Vec<BrowserCurrentHost>,
    pub physical_hosts: BrowserPhysicalHostSummary,
    pub reader: BrowserPatchbayReader,
    pub latest_sequence: u64,
    pub latest_sign_id: SignId,
    pub salient_action: BrowserLifecycleAction,
    pub status_line: String,
    pub placement_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserBodyLifecycle {
    Lulled,
    Awake { wake_id: WakeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserCurrentHost {
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observation_sequence: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserPhysicalHostSummary {
    NotEvidenced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserPatchbayReader {
    HostedByBody {
        plan_id: PlanId,
        implementation_id: ImplementationId,
    },
    ExternalReadingHostedBody {
        hosted_plan_id: PlanId,
        hosted_implementation_id: ImplementationId,
    },
    ExternalReadingUnhostedBody,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserLifecycleAction {
    Wake,
    Lull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserBodyHistory {
    pub body_id: BodyId,
    pub place: PresentationPlace,
    pub aspect: PresentationAspect,
    pub exact_depth: PresentationDepth,
    pub alternate_manifestation: BrowserHistoryManifestation,
    pub entries: Vec<BrowserBodyHistoryEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserHistoryManifestation {
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserBodyHistoryEntry {
    pub evidence_sequence: u64,
    pub title: String,
    pub narrative: String,
    pub exact_body_id: BodyId,
    pub exact_record: BodyBiographyRecord,
    pub inspect_sign_id: SignId,
    pub inspect_subject: String,
    pub inspect_place: PresentationPlace,
    pub inspect_aspect: PresentationAspect,
    pub inspect_depth: PresentationDepth,
    pub linear: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserBodyWorkbenchError {
    MismatchedRevision,
    MismatchedBody,
    InvalidProjection,
    CapacityExceeded,
}

impl BrowserBodyWorkbench {
    pub fn from_models(
        current: &CurrentBodyFrame,
        history: &ReadableBodyHistory,
    ) -> Result<Self, BrowserBodyWorkbenchError> {
        if current.evidence_revision != history.evidence_revision {
            return Err(BrowserBodyWorkbenchError::MismatchedRevision);
        }
        if current.body_id != history.body_id {
            return Err(BrowserBodyWorkbenchError::MismatchedBody);
        }
        let value = Self {
            schema: "conduit.patchbay/browser-body-workbench@1".into(),
            evidence_revision: current.evidence_revision,
            current: BrowserCurrentBody {
                body_id: current.body_id.clone(),
                friendly_name: current.friendly_name.clone(),
                program_label: current.program.label.clone(),
                source_document_id: current.program.source_document_id.clone(),
                checked_form_id: current.program.checked_form_id.clone(),
                lifecycle: match &current.lifecycle {
                    CurrentBodyLifecycle::Lulled => BrowserBodyLifecycle::Lulled,
                    CurrentBodyLifecycle::Awake { wake_id } => BrowserBodyLifecycle::Awake {
                        wake_id: wake_id.clone(),
                    },
                },
                admitted_parts: current.admitted_parts,
                current_hosts: current
                    .current_hosts
                    .iter()
                    .map(|host| BrowserCurrentHost {
                        part_id: host.part_id.clone(),
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        offer_generation: host.offer_generation,
                        observation_sequence: host.observation_sequence,
                    })
                    .collect(),
                physical_hosts: match current.physical_hosts {
                    CurrentBodyPhysicalHostSummary::NotEvidenced => {
                        BrowserPhysicalHostSummary::NotEvidenced
                    }
                },
                reader: reader(&current.patchbay_reader),
                latest_sequence: current.latest_evidence.sequence,
                latest_sign_id: current.latest_evidence.sign_id.clone(),
                salient_action: match current.salient_action {
                    CurrentBodyLifecycleAction::Wake => BrowserLifecycleAction::Wake,
                    CurrentBodyLifecycleAction::Lull => BrowserLifecycleAction::Lull,
                },
                status_line: current.status_line.clone(),
                placement_line: current.placement_line.into(),
            },
            history: BrowserBodyHistory {
                body_id: history.body_id.clone(),
                place: history.place,
                aspect: history.aspect,
                exact_depth: history.access.exact_evidence_depth,
                alternate_manifestation: match history.access.alternate_manifestation {
                    BodyHistoryManifestation::Linear => BrowserHistoryManifestation::Linear,
                },
                entries: history
                    .entries
                    .iter()
                    .map(|entry| BrowserBodyHistoryEntry {
                        evidence_sequence: match entry.moment {
                            BodyHistoryMoment::EvidenceSequence(sequence) => sequence,
                        },
                        title: entry.title.into(),
                        narrative: entry.narrative.clone(),
                        exact_body_id: entry.exact.body_id.clone(),
                        exact_record: entry.exact.record.clone(),
                        inspect_sign_id: entry.inspect.sign_id.clone(),
                        inspect_subject: entry.inspect.subject_identity.clone(),
                        inspect_place: entry.inspect.place,
                        inspect_aspect: entry.inspect.aspect,
                        inspect_depth: entry.inspect.depth,
                        linear: entry.linear.clone(),
                    })
                    .collect(),
            },
        };
        value.validate_against(Some(&current.body_id))?;
        Ok(value)
    }

    pub fn validate_against(
        &self,
        expected_body: Option<&BodyId>,
    ) -> Result<(), BrowserBodyWorkbenchError> {
        if self.schema != "conduit.patchbay/browser-body-workbench@1" || self.evidence_revision == 0
        {
            return Err(BrowserBodyWorkbenchError::InvalidProjection);
        }
        if expected_body != Some(&self.current.body_id)
            || self.current.body_id != self.history.body_id
        {
            return Err(BrowserBodyWorkbenchError::MismatchedBody);
        }
        if self.current.current_hosts.len() > MAX_BODY_BIOGRAPHY_RECORDS
            || self.history.entries.is_empty()
            || self.history.entries.len() > MAX_BODY_BIOGRAPHY_RECORDS
        {
            return Err(BrowserBodyWorkbenchError::CapacityExceeded);
        }
        if self.history.place != PresentationPlace::Body
            || self.history.aspect != PresentationAspect::Signs
            || self.history.exact_depth != PresentationDepth::Exact
            || self.history.entries.iter().any(|entry| {
                entry.title.is_empty()
                    || entry.title.len() > MAX_BODY_HISTORY_TITLE_BYTES
                    || entry.narrative.len() > MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES
                    || entry.linear.len() > MAX_BODY_HISTORY_LINEAR_BYTES
                    || entry.evidence_sequence != entry.exact_record.sequence
                    || entry.exact_body_id != self.current.body_id
                    || entry.inspect_sign_id != entry.exact_record.sign_id
                    || entry.inspect_subject
                        != format!("sign/{}", entry.exact_record.sign_id.as_str())
                    || entry.inspect_place != PresentationPlace::Body
                    || entry.inspect_aspect != PresentationAspect::Signs
                    || entry.inspect_depth != PresentationDepth::Exact
            })
        {
            return Err(BrowserBodyWorkbenchError::InvalidProjection);
        }
        Ok(())
    }
}

fn reader(value: &CurrentBodyPatchbayReader) -> BrowserPatchbayReader {
    match value {
        CurrentBodyPatchbayReader::HostedByBody {
            plan_id,
            implementation_id,
        } => BrowserPatchbayReader::HostedByBody {
            plan_id: plan_id.clone(),
            implementation_id: implementation_id.clone(),
        },
        CurrentBodyPatchbayReader::ExternalReadingHostedBody {
            hosted_plan_id,
            hosted_implementation_id,
        } => BrowserPatchbayReader::ExternalReadingHostedBody {
            hosted_plan_id: hosted_plan_id.clone(),
            hosted_implementation_id: hosted_implementation_id.clone(),
        },
        CurrentBodyPatchbayReader::ExternalReadingUnhostedBody => {
            BrowserPatchbayReader::ExternalReadingUnhostedBody
        }
    }
}
