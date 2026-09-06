use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    BootId, CheckedFormId, HostId, ImplementationId, PlanId, SignId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::{
    Body, BodyId, BodyLifecycleEvent, BodyMembership, MembershipChangeId, MembershipEvent,
    MembershipEventKind, PartId, MAX_LIFECYCLE_ID_BYTES,
};

mod validation;
mod wake_history;

pub const MAX_BODY_BIOGRAPHY_RECORDS: usize = 64;
pub const MAX_BODY_BIOGRAPHY_WAKES: usize = 8;
pub const MAX_BODY_FRIENDLY_NAME_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyGraduationChoice {
    HostedPatchbay,
    ExternalReader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyGraduationEvidence {
    pub body_id: BodyId,
    pub sequence: u64,
    pub sign_id: SignId,
    pub choice: BodyGraduationChoice,
    pub patchbay_plan_id: Option<PlanId>,
    pub patchbay_implementation_id: Option<ImplementationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyBiographyRecordKind {
    WakeEvent {
        wake_id: crate::WakeId,
        event_index: u16,
    },
    LullRetained {
        wake_id: crate::WakeId,
    },
    Born {
        initial_workset: crate::BodyWorkset,
        workload_revision: u64,
    },
    PartAdmitted {
        change_id: MembershipChangeId,
        part_id: PartId,
    },
    HostJoined {
        change_id: MembershipChangeId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
    },
    HostLeft {
        change_id: MembershipChangeId,
        part_id: PartId,
        prior_boot_id: BootId,
    },
    PartRevoked {
        change_id: MembershipChangeId,
        part_id: PartId,
    },
    FormAdmitted {
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        workload_revision: u64,
    },
    FormRemoved {
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        workload_revision: u64,
    },
    Graduated {
        choice: BodyGraduationChoice,
        patchbay_plan_id: Option<PlanId>,
        patchbay_implementation_id: Option<ImplementationId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBiographyRecord {
    pub sequence: u64,
    pub sign_id: SignId,
    pub kind: BodyBiographyRecordKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyBiographyEvidence {
    pub schema: String,
    pub body_id: BodyId,
    pub friendly_name: String,
    pub body: Body,
    pub membership: BodyMembership,
    pub graduation: Option<BodyGraduationEvidence>,
    pub records: Vec<BodyBiographyRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wakes: Vec<crate::Wake>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyBiographyError {
    InvalidBody,
    WrongBody,
    InvalidMetadata,
    InvalidSequence,
    InvalidEvidence,
    DuplicateEvidence,
    CapacityExhausted,
}

impl BodyBiographyEvidence {
    pub fn born(
        body: Body,
        membership: BodyMembership,
        friendly_name: String,
    ) -> Result<Self, BodyBiographyError> {
        let sign_id = match body.events.first() {
            Some(BodyLifecycleEvent::Born { sign_id, .. }) => sign_id.clone(),
            _ => return Err(BodyBiographyError::InvalidBody),
        };
        let initial_workset = match body.events.first() {
            Some(BodyLifecycleEvent::Born {
                initial_workset, ..
            }) => initial_workset.clone(),
            _ => return Err(BodyBiographyError::InvalidBody),
        };
        let evidence = Self {
            schema: "conduit.body/biography-evidence@2".into(),
            body_id: body.body_id.clone(),
            friendly_name,
            membership,
            graduation: None,
            wakes: Vec::new(),
            records: vec![BodyBiographyRecord {
                sequence: body.birth_sequence,
                sign_id,
                kind: BodyBiographyRecordKind::Born {
                    initial_workset,
                    workload_revision: 0,
                },
            }],
            body,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn can_append(&self, count: usize) -> Result<(), BodyBiographyError> {
        if self
            .records
            .len()
            .checked_add(count)
            .is_some_and(|total| total <= MAX_BODY_BIOGRAPHY_RECORDS)
        {
            Ok(())
        } else {
            Err(BodyBiographyError::CapacityExhausted)
        }
    }

    pub fn append_membership_events(
        &mut self,
        membership: BodyMembership,
        events: &[(MembershipChangeId, u64)],
    ) -> Result<(), BodyBiographyError> {
        self.can_append(events.len())?;
        if membership.body_id != self.body_id {
            return Err(BodyBiographyError::WrongBody);
        }
        let mut candidate = self.clone();
        candidate.membership = membership;
        for (change_id, sequence) in events {
            let event = membership_event(&candidate.membership, change_id)?.clone();
            if event.body_id != candidate.body_id || *sequence <= candidate.last_sequence() {
                return Err(BodyBiographyError::InvalidSequence);
            }
            if candidate.records.iter().any(|record| match &record.kind {
                BodyBiographyRecordKind::PartAdmitted { change_id, .. }
                | BodyBiographyRecordKind::HostJoined { change_id, .. }
                | BodyBiographyRecordKind::HostLeft { change_id, .. }
                | BodyBiographyRecordKind::PartRevoked { change_id, .. } => {
                    change_id == &event.change_id
                }
                _ => false,
            }) {
                return Err(BodyBiographyError::DuplicateEvidence);
            }
            let kind = match &event.kind {
                MembershipEventKind::Admitted { .. } => BodyBiographyRecordKind::PartAdmitted {
                    change_id: event.change_id.clone(),
                    part_id: event.part_id.clone(),
                },
                MembershipEventKind::HostAttached { observation } => {
                    BodyBiographyRecordKind::HostJoined {
                        change_id: event.change_id.clone(),
                        part_id: event.part_id.clone(),
                        host_id: observation.host_id.clone(),
                        boot_id: observation.boot_id.clone(),
                    }
                }
                MembershipEventKind::HostDetached { prior_boot_id } => {
                    BodyBiographyRecordKind::HostLeft {
                        change_id: event.change_id.clone(),
                        part_id: event.part_id.clone(),
                        prior_boot_id: prior_boot_id.clone(),
                    }
                }
                MembershipEventKind::Revoked => BodyBiographyRecordKind::PartRevoked {
                    change_id: event.change_id.clone(),
                    part_id: event.part_id.clone(),
                },
            };
            candidate.records.push(BodyBiographyRecord {
                sequence: *sequence,
                sign_id: event.sign_id,
                kind,
            });
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Appends exact biography records for Body workload changes and replaces
    /// current Body truth atomically. `events` pairs each workload event Sign
    /// with its monotonically increasing biography sequence.
    pub fn append_body_workload_events(
        &mut self,
        body: Body,
        events: &[(SignId, u64)],
    ) -> Result<(), BodyBiographyError> {
        self.can_append(events.len())?;
        if body.body_id != self.body_id {
            return Err(BodyBiographyError::WrongBody);
        }
        if !body.events.starts_with(&self.body.events) {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let mut candidate = self.clone();
        candidate.body = body;
        for (sign_id, sequence) in events {
            if *sequence <= candidate.last_sequence()
                || candidate
                    .records
                    .iter()
                    .any(|record| &record.sign_id == sign_id)
            {
                return Err(BodyBiographyError::InvalidSequence);
            }
            let event = candidate
                .body
                .events
                .iter()
                .find(|event| event.sign_id() == sign_id)
                .ok_or(BodyBiographyError::InvalidEvidence)?;
            let kind = match event {
                BodyLifecycleEvent::FormAdmitted {
                    source_document_id,
                    checked_form_id,
                    workload_revision,
                    ..
                } => BodyBiographyRecordKind::FormAdmitted {
                    source_document_id: source_document_id.clone(),
                    checked_form_id: checked_form_id.clone(),
                    workload_revision: *workload_revision,
                },
                BodyLifecycleEvent::FormRemoved {
                    source_document_id,
                    checked_form_id,
                    workload_revision,
                    ..
                } => BodyBiographyRecordKind::FormRemoved {
                    source_document_id: source_document_id.clone(),
                    checked_form_id: checked_form_id.clone(),
                    workload_revision: *workload_revision,
                },
                _ => return Err(BodyBiographyError::InvalidEvidence),
            };
            candidate.records.push(BodyBiographyRecord {
                sequence: *sequence,
                sign_id: sign_id.clone(),
                kind,
            });
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn graduate(&mut self, evidence: BodyGraduationEvidence) -> Result<(), BodyBiographyError> {
        self.can_append(1)?;
        if self.graduation.is_some()
            || evidence.body_id != self.body_id
            || evidence.sequence <= self.last_sequence()
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        validate_graduation(&evidence)?;
        self.records.push(BodyBiographyRecord {
            sequence: evidence.sequence,
            sign_id: evidence.sign_id.clone(),
            kind: BodyBiographyRecordKind::Graduated {
                choice: evidence.choice.clone(),
                patchbay_plan_id: evidence.patchbay_plan_id.clone(),
                patchbay_implementation_id: evidence.patchbay_implementation_id.clone(),
            },
        });
        self.graduation = Some(evidence);
        self.validate_records()
    }

    pub fn replace_membership(
        &mut self,
        membership: BodyMembership,
    ) -> Result<(), BodyBiographyError> {
        if membership.body_id != self.body_id {
            return Err(BodyBiographyError::WrongBody);
        }
        self.membership = membership;
        self.validate()
    }

    fn last_sequence(&self) -> u64 {
        self.records.last().map_or(0, |record| record.sequence)
    }
}

fn membership_event<'a>(
    membership: &'a BodyMembership,
    change_id: &MembershipChangeId,
) -> Result<&'a MembershipEvent, BodyBiographyError> {
    membership
        .events
        .iter()
        .find(|event| &event.change_id == change_id)
        .ok_or(BodyBiographyError::InvalidEvidence)
}

fn validate_graduation(evidence: &BodyGraduationEvidence) -> Result<(), BodyBiographyError> {
    if evidence.sequence == 0
        || evidence.sign_id.as_str().is_empty()
        || evidence.sign_id.as_str().len() > MAX_LIFECYCLE_ID_BYTES
    {
        return Err(BodyBiographyError::InvalidEvidence);
    }
    match evidence.choice {
        BodyGraduationChoice::HostedPatchbay
            if evidence.patchbay_plan_id.is_some()
                && evidence.patchbay_implementation_id.is_some() =>
        {
            Ok(())
        }
        BodyGraduationChoice::ExternalReader
            if evidence.patchbay_plan_id.is_none()
                && evidence.patchbay_implementation_id.is_none() =>
        {
            Ok(())
        }
        _ => Err(BodyBiographyError::InvalidEvidence),
    }
}
