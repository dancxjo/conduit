use alloc::{string::String, vec, vec::Vec};
use conduit_core::{BootId, HostId, ImplementationId, PlanId, SignId};
use serde::{Deserialize, Serialize};

use crate::{
    Body, BodyId, BodyLifecycleEvent, BodyMembership, MembershipChangeId, MembershipEvent,
    MembershipEventKind, PartId, SeedId, MAX_LIFECYCLE_ID_BYTES,
};

pub const MAX_BODY_BIOGRAPHY_RECORDS: usize = 64;
pub const MAX_BODY_FRIENDLY_NAME_BYTES: usize = 64;
pub const MAX_BODY_PROGRAM_ID_BYTES: usize = 128;

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
    Born {
        seed_id: SeedId,
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
    /// Historical display label for the Seed Form at BIRTH. This is not the
    /// current workload and does not create a distinct Program identity.
    pub initial_program: String,
    pub body: Body,
    pub membership: BodyMembership,
    pub graduation: Option<BodyGraduationEvidence>,
    pub records: Vec<BodyBiographyRecord>,
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
    pub fn seed_form_label(&self) -> &str {
        &self.initial_program
    }

    pub fn born(
        body: Body,
        membership: BodyMembership,
        friendly_name: String,
        initial_program: String,
    ) -> Result<Self, BodyBiographyError> {
        let sign_id = match body.events.first() {
            Some(BodyLifecycleEvent::Born { sign_id }) => sign_id.clone(),
            _ => return Err(BodyBiographyError::InvalidBody),
        };
        let evidence = Self {
            schema: "conduit.body/biography-evidence@1".into(),
            body_id: body.body_id.clone(),
            friendly_name,
            initial_program,
            membership,
            graduation: None,
            records: vec![BodyBiographyRecord {
                sequence: body.birth_sequence,
                sign_id,
                kind: BodyBiographyRecordKind::Born {
                    seed_id: body.seed_id.clone(),
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
                | BodyBiographyRecordKind::HostJoined { change_id, .. } => {
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
                MembershipEventKind::HostDetached { .. } | MembershipEventKind::Revoked => {
                    return Err(BodyBiographyError::InvalidEvidence)
                }
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

    pub fn validate(&self) -> Result<(), BodyBiographyError> {
        self.body
            .validate()
            .map_err(|_| BodyBiographyError::InvalidBody)?;
        self.membership
            .validate()
            .map_err(|_| BodyBiographyError::InvalidEvidence)?;
        if self.schema != "conduit.body/biography-evidence@1"
            || self.body_id != self.body.body_id
            || self.body_id != self.membership.body_id
            || self.friendly_name.trim().is_empty()
            || self.friendly_name.len() > MAX_BODY_FRIENDLY_NAME_BYTES
            || self.initial_program.is_empty()
            || self.initial_program.len() > MAX_BODY_PROGRAM_ID_BYTES
        {
            return Err(BodyBiographyError::InvalidMetadata);
        }
        if let Some(graduation) = &self.graduation {
            validate_graduation(graduation)?;
        }
        self.validate_records()
    }

    fn last_sequence(&self) -> u64 {
        self.records.last().map_or(0, |record| record.sequence)
    }

    fn validate_records(&self) -> Result<(), BodyBiographyError> {
        if self.records.is_empty() || self.records.len() > MAX_BODY_BIOGRAPHY_RECORDS {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let first = &self.records[0];
        if first.sequence != self.body.birth_sequence
            || !matches!(&first.kind, BodyBiographyRecordKind::Born { seed_id } if seed_id == &self.body.seed_id)
            || first.sign_id != self.body.sign_ids[0]
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        if self
            .records
            .windows(2)
            .any(|pair| pair[0].sequence >= pair[1].sequence)
        {
            return Err(BodyBiographyError::InvalidSequence);
        }
        for record in self.records.iter().skip(1) {
            match &record.kind {
                BodyBiographyRecordKind::PartAdmitted { change_id, part_id } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(event.kind, MembershipEventKind::Admitted { .. })
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::HostJoined {
                    change_id,
                    part_id,
                    host_id,
                    boot_id,
                } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(&event.kind, MembershipEventKind::HostAttached { observation } if &observation.host_id == host_id && &observation.boot_id == boot_id)
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::Graduated {
                    choice,
                    patchbay_plan_id,
                    patchbay_implementation_id,
                } => {
                    let graduation = self
                        .graduation
                        .as_ref()
                        .ok_or(BodyBiographyError::InvalidEvidence)?;
                    if graduation.sequence != record.sequence
                        || graduation.sign_id != record.sign_id
                        || &graduation.choice != choice
                        || &graduation.patchbay_plan_id != patchbay_plan_id
                        || &graduation.patchbay_implementation_id != patchbay_implementation_id
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::Born { .. } => {
                    return Err(BodyBiographyError::DuplicateEvidence)
                }
            }
        }
        let membership_records = self
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    BodyBiographyRecordKind::PartAdmitted { .. }
                        | BodyBiographyRecordKind::HostJoined { .. }
                )
            })
            .count();
        if membership_records != self.membership.events.len()
            || self.graduation.is_some()
                != self
                    .records
                    .iter()
                    .any(|record| matches!(record.kind, BodyBiographyRecordKind::Graduated { .. }))
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
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
