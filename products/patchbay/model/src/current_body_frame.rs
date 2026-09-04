//! Human-first current-Body facts for an attached ordinary Patchbay.
//!
//! This frame is a bounded projection of already-validated biography evidence.
//! It does not discover Hosts, classify hardware, plan placement, or retain a
//! competing copy of Body truth.

use conduit_body::{
    BodyBiographyRecordKind, BodyGraduationChoice, BodyState, MembershipState, PartId, WakeId,
};
use conduit_core::{
    BootId, CheckedFormId, HostId, ImplementationId, OfferGeneration, PlanId, SignId,
    SourceDocumentId,
};
use serde::Serialize;

use crate::{PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, PatchbayBodyEntranceError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentBodyFrame {
    pub schema: &'static str,
    pub evidence_revision: u64,
    pub body_id: conduit_body::BodyId,
    pub friendly_name: String,
    pub active_forms: Vec<CurrentBodyForm>,
    pub lifecycle: CurrentBodyLifecycle,
    pub admitted_parts: usize,
    pub current_hosts: Vec<CurrentBodyHost>,
    pub physical_hosts: CurrentBodyPhysicalHostSummary,
    pub patchbay_reader: CurrentBodyPatchbayReader,
    pub latest_evidence: CurrentBodyTransition,
    pub salient_action: CurrentBodyLifecycleAction,
    pub status_line: String,
    pub placement_line: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentBodyForm {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CurrentBodyLifecycle {
    Lulled,
    Awake { wake_id: WakeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentBodyHost {
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observation_sequence: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub enum CurrentBodyPhysicalHostSummary {
    /// Biography evidence records membership and current Host/Boot presence,
    /// but does not classify a Host as physical.
    NotEvidenced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CurrentBodyPatchbayReader {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentBodyTransition {
    pub sequence: u64,
    pub sign_id: SignId,
    pub kind: BodyBiographyRecordKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub enum CurrentBodyLifecycleAction {
    Wake,
    Lull,
}

impl CurrentBodyFrame {
    pub fn from_attachment(evidence_revision: u64, attachment: &PatchbayBodyAttachment) -> Self {
        let evidence = attachment.evidence();
        let lifecycle = match &evidence.body.state {
            BodyState::Lulled => CurrentBodyLifecycle::Lulled,
            BodyState::Awake { wake_id } => CurrentBodyLifecycle::Awake {
                wake_id: wake_id.clone(),
            },
        };
        let salient_action = match lifecycle {
            CurrentBodyLifecycle::Lulled => CurrentBodyLifecycleAction::Wake,
            CurrentBodyLifecycle::Awake { .. } => CurrentBodyLifecycleAction::Lull,
        };
        let current_hosts = evidence
            .membership
            .parts
            .iter()
            .filter(|part| part.state == MembershipState::Admitted)
            .filter_map(|part| {
                part.current.as_ref().map(|current| CurrentBodyHost {
                    part_id: part.part_id.clone(),
                    host_id: current.host_id.clone(),
                    boot_id: current.boot_id.clone(),
                    offer_generation: current.offer_generation,
                    observation_sequence: current.sequence,
                })
            })
            .collect::<Vec<_>>();
        let admitted_parts = evidence
            .membership
            .parts
            .iter()
            .filter(|part| part.state == MembershipState::Admitted)
            .count();
        let patchbay_reader = reader(attachment);
        let placement_line = match patchbay_reader {
            CurrentBodyPatchbayReader::HostedByBody { .. } => {
                "Patchbay is hosted by this Body through its exact graduation placement."
            }
            CurrentBodyPatchbayReader::ExternalReadingHostedBody { .. } => {
                "This external Patchbay is reading a Body that also retained a hosted Patchbay placement."
            }
            CurrentBodyPatchbayReader::ExternalReadingUnhostedBody => {
                "This external Patchbay is reading a Body that graduated without a hosted Patchbay."
            }
        };
        let lifecycle_label = match lifecycle {
            CurrentBodyLifecycle::Lulled => "Lulled",
            CurrentBodyLifecycle::Awake { .. } => "Awake",
        };
        let status_line = format!(
            "{lifecycle_label} · {} {} · {} current {} · physical Host classification not evidenced",
            admitted_parts,
            plural(admitted_parts, "Part", "Parts"),
            current_hosts.len(),
            plural(current_hosts.len(), "Host", "Hosts")
        );
        let latest = evidence
            .records
            .last()
            .expect("validated biography evidence always has a record");

        Self {
            schema: "conduit.patchbay/current-body-frame@1",
            evidence_revision,
            body_id: evidence.body_id.clone(),
            friendly_name: evidence.friendly_name.clone(),
            active_forms: evidence
                .body
                .workset
                .forms()
                .iter()
                .map(|form| CurrentBodyForm {
                    source_document_id: form.source_document_id.clone(),
                    checked_form_id: form.checked_form_id.clone(),
                })
                .collect(),
            lifecycle,
            admitted_parts,
            current_hosts,
            physical_hosts: CurrentBodyPhysicalHostSummary::NotEvidenced,
            patchbay_reader,
            latest_evidence: CurrentBodyTransition {
                sequence: latest.sequence,
                sign_id: latest.sign_id.clone(),
                kind: latest.kind.clone(),
            },
            salient_action,
            status_line,
            placement_line,
        }
    }
}

fn reader(attachment: &PatchbayBodyAttachment) -> CurrentBodyPatchbayReader {
    match attachment.entrance() {
        PatchbayBodyApplicationEntrance::Hosted {
            plan_id,
            implementation_id,
        } => CurrentBodyPatchbayReader::HostedByBody {
            plan_id: plan_id.clone(),
            implementation_id: implementation_id.clone(),
        },
        PatchbayBodyApplicationEntrance::ExternalReader => {
            let graduation = attachment
                .evidence()
                .graduation
                .as_ref()
                .expect("ordinary Patchbay attachment requires graduation");
            match graduation.choice {
                BodyGraduationChoice::HostedPatchbay => {
                    CurrentBodyPatchbayReader::ExternalReadingHostedBody {
                        hosted_plan_id: graduation
                            .patchbay_plan_id
                            .clone()
                            .expect("validated hosted graduation has a Plan"),
                        hosted_implementation_id: graduation
                            .patchbay_implementation_id
                            .clone()
                            .expect("validated hosted graduation has an implementation"),
                    }
                }
                BodyGraduationChoice::ExternalReader => {
                    CurrentBodyPatchbayReader::ExternalReadingUnhostedBody
                }
            }
        }
    }
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 {
        one
    } else {
        many
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentBodyFrameError {
    InvalidRevision,
    StaleRevision { current: u64, offered: u64 },
    Entrance(PatchbayBodyEntranceError),
}

#[derive(Debug, Default)]
pub struct CurrentBodyFrameSlot {
    last_revision: Option<u64>,
    current: Option<CurrentBodyFrame>,
}

impl CurrentBodyFrameSlot {
    pub fn current(&self) -> Option<&CurrentBodyFrame> {
        self.current.as_ref()
    }

    pub fn replace_serialized(
        &mut self,
        revision: u64,
        encoded: &[u8],
        entrance: PatchbayBodyApplicationEntrance,
    ) -> Result<&CurrentBodyFrame, CurrentBodyFrameError> {
        if revision == 0 {
            self.current = None;
            return Err(CurrentBodyFrameError::InvalidRevision);
        }
        if let Some(current) = self.last_revision {
            if revision <= current {
                self.current = None;
                return Err(CurrentBodyFrameError::StaleRevision {
                    current,
                    offered: revision,
                });
            }
        }
        self.last_revision = Some(revision);
        self.current = None;
        let attachment = PatchbayBodyAttachment::open_serialized(encoded, entrance)
            .map_err(CurrentBodyFrameError::Entrance)?;
        self.current = Some(CurrentBodyFrame::from_attachment(revision, &attachment));
        Ok(self.current.as_ref().expect("current frame was installed"))
    }
}

#[cfg(test)]
mod tests;
