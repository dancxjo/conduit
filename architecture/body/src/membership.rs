use alloc::vec::Vec;
use conduit_core::{BootId, HostId, OfferGeneration, SignId};
use serde::{Deserialize, Serialize};

use crate::identity::{bind_identity, validate_ids};
use crate::{BodyId, BodyLifecycleError, MembershipChangeId, MembershipProofId, PartId};

pub const MAX_BODY_PARTS: usize = 16;
pub const MAX_MEMBERSHIP_EVENTS: usize = 64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyMembershipRevision(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipState {
    Admitted,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedHostObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    /// Opaque reference to verified admission/continuity evidence, never secret material.
    pub proof_id: MembershipProofId,
    /// Monotonic observation sequence within this Part relationship.
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartMembership {
    pub part_id: PartId,
    pub state: MembershipState,
    pub current: Option<AuthenticatedHostObservation>,
}

impl PartMembership {
    pub fn is_present(&self) -> bool {
        self.state == MembershipState::Admitted && self.current.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipEventKind {
    Admitted {
        proof_id: MembershipProofId,
    },
    HostAttached {
        observation: AuthenticatedHostObservation,
    },
    HostDetached {
        prior_boot_id: BootId,
    },
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipEvent {
    pub change_id: MembershipChangeId,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub revision: BodyMembershipRevision,
    pub sign_id: SignId,
    pub kind: MembershipEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyMembership {
    pub body_id: BodyId,
    pub revision: BodyMembershipRevision,
    pub parts: Vec<PartMembership>,
    pub events: Vec<MembershipEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MembershipRefusal {
    EmptyIdentity,
    IdentityTooLong,
    WrongBody,
    StaleRevision,
    DuplicatePart,
    UnknownPart,
    RevokedPart,
    DuplicateChange,
    DuplicateSign,
    PartCapacityExhausted,
    EventCapacityExhausted,
    StaleObservation,
    StaleOfferGeneration,
    ObservationMismatch,
    MalformedState,
}

impl From<BodyLifecycleError> for MembershipRefusal {
    fn from(value: BodyLifecycleError) -> Self {
        match value {
            BodyLifecycleError::EmptyIdentity => Self::EmptyIdentity,
            BodyLifecycleError::IdentityTooLong => Self::IdentityTooLong,
            _ => Self::MalformedState,
        }
    }
}

impl BodyMembership {
    pub fn new(body_id: BodyId) -> Result<Self, MembershipRefusal> {
        validate_ids(&[body_id.as_str()])?;
        Ok(Self {
            body_id,
            revision: BodyMembershipRevision(0),
            parts: Vec::new(),
            events: Vec::new(),
        })
    }

    pub fn admit(
        &mut self,
        body_id: &BodyId,
        expected_revision: BodyMembershipRevision,
        part_id: PartId,
        proof_id: MembershipProofId,
        sign_id: SignId,
    ) -> Result<MembershipChangeId, MembershipRefusal> {
        self.validate_request(body_id, expected_revision, &part_id, &sign_id)?;
        validate_ids(&[proof_id.as_str()])?;
        if self.parts.iter().any(|part| part.part_id == part_id) {
            return Err(MembershipRefusal::DuplicatePart);
        }
        if self.parts.len() == MAX_BODY_PARTS {
            return Err(MembershipRefusal::PartCapacityExhausted);
        }
        let revision = self.next_revision()?;
        let change_id = self.change_id(&part_id, &sign_id, revision);
        self.parts.push(PartMembership {
            part_id: part_id.clone(),
            state: MembershipState::Admitted,
            current: None,
        });
        self.push_event(MembershipEvent {
            change_id: change_id.clone(),
            body_id: self.body_id.clone(),
            part_id,
            revision,
            sign_id,
            kind: MembershipEventKind::Admitted { proof_id },
        });
        self.revision = revision;
        Ok(change_id)
    }

    pub fn observe_present(
        &mut self,
        body_id: &BodyId,
        expected_revision: BodyMembershipRevision,
        part_id: &PartId,
        observation: AuthenticatedHostObservation,
        sign_id: SignId,
    ) -> Result<MembershipChangeId, MembershipRefusal> {
        self.validate_request(body_id, expected_revision, part_id, &sign_id)?;
        validate_observation(&observation)?;
        let index = self.part_index(part_id)?;
        let part = &self.parts[index];
        if part.state != MembershipState::Admitted {
            return Err(MembershipRefusal::RevokedPart);
        }
        if let Some(current) = &part.current {
            if observation.sequence <= current.sequence {
                return Err(MembershipRefusal::StaleObservation);
            }
            if observation.host_id == current.host_id
                && observation.boot_id == current.boot_id
                && observation.offer_generation <= current.offer_generation
            {
                return Err(MembershipRefusal::StaleOfferGeneration);
            }
        }
        let revision = self.next_revision()?;
        let change_id = self.change_id(part_id, &sign_id, revision);
        self.parts[index].current = Some(observation.clone());
        self.push_event(MembershipEvent {
            change_id: change_id.clone(),
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            revision,
            sign_id,
            kind: MembershipEventKind::HostAttached { observation },
        });
        self.revision = revision;
        Ok(change_id)
    }

    pub fn observe_offline(
        &mut self,
        body_id: &BodyId,
        expected_revision: BodyMembershipRevision,
        part_id: &PartId,
        prior_boot_id: &BootId,
        sign_id: SignId,
    ) -> Result<MembershipChangeId, MembershipRefusal> {
        self.validate_request(body_id, expected_revision, part_id, &sign_id)?;
        validate_ids(&[prior_boot_id.as_str()])?;
        let index = self.part_index(part_id)?;
        let part = &self.parts[index];
        if part.state != MembershipState::Admitted {
            return Err(MembershipRefusal::RevokedPart);
        }
        if part.current.as_ref().map(|current| &current.boot_id) != Some(prior_boot_id) {
            return Err(MembershipRefusal::ObservationMismatch);
        }
        let revision = self.next_revision()?;
        let change_id = self.change_id(part_id, &sign_id, revision);
        self.parts[index].current = None;
        self.push_event(MembershipEvent {
            change_id: change_id.clone(),
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            revision,
            sign_id,
            kind: MembershipEventKind::HostDetached {
                prior_boot_id: prior_boot_id.clone(),
            },
        });
        self.revision = revision;
        Ok(change_id)
    }

    pub fn revoke(
        &mut self,
        body_id: &BodyId,
        expected_revision: BodyMembershipRevision,
        part_id: &PartId,
        sign_id: SignId,
    ) -> Result<MembershipChangeId, MembershipRefusal> {
        self.validate_request(body_id, expected_revision, part_id, &sign_id)?;
        let index = self.part_index(part_id)?;
        if self.parts[index].state == MembershipState::Revoked {
            return Err(MembershipRefusal::RevokedPart);
        }
        let revision = self.next_revision()?;
        let change_id = self.change_id(part_id, &sign_id, revision);
        self.parts[index].state = MembershipState::Revoked;
        self.parts[index].current = None;
        self.push_event(MembershipEvent {
            change_id: change_id.clone(),
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            revision,
            sign_id,
            kind: MembershipEventKind::Revoked,
        });
        self.revision = revision;
        Ok(change_id)
    }

    pub fn validate(&self) -> Result<(), MembershipRefusal> {
        validate_ids(&[self.body_id.as_str()])?;
        if self.parts.len() > MAX_BODY_PARTS || self.events.len() > MAX_MEMBERSHIP_EVENTS {
            return Err(MembershipRefusal::MalformedState);
        }
        if self.events.len() as u64 != self.revision.0 {
            return Err(MembershipRefusal::MalformedState);
        }
        for (index, event) in self.events.iter().enumerate() {
            if event.body_id != self.body_id
                || event.revision != BodyMembershipRevision(index as u64 + 1)
                || event.change_id != self.change_id(&event.part_id, &event.sign_id, event.revision)
            {
                return Err(MembershipRefusal::MalformedState);
            }
            validate_ids(&[
                event.part_id.as_str(),
                event.sign_id.as_str(),
                event.change_id.as_str(),
            ])?;
            if self.events[..index]
                .iter()
                .any(|prior| prior.change_id == event.change_id)
            {
                return Err(MembershipRefusal::DuplicateChange);
            }
            if self.events[..index]
                .iter()
                .any(|prior| prior.sign_id == event.sign_id)
            {
                return Err(MembershipRefusal::DuplicateSign);
            }
        }
        let replayed = replay_events(&self.body_id, &self.events)?;
        if replayed != self.parts {
            return Err(MembershipRefusal::MalformedState);
        }
        for (index, part) in self.parts.iter().enumerate() {
            validate_ids(&[part.part_id.as_str()])?;
            if self.parts[..index]
                .iter()
                .any(|prior| prior.part_id == part.part_id)
                || (part.state == MembershipState::Revoked && part.current.is_some())
            {
                return Err(MembershipRefusal::MalformedState);
            }
            if let Some(observation) = &part.current {
                validate_observation(observation)?;
            }
        }
        Ok(())
    }

    fn validate_request(
        &self,
        body_id: &BodyId,
        expected_revision: BodyMembershipRevision,
        part_id: &PartId,
        sign_id: &SignId,
    ) -> Result<(), MembershipRefusal> {
        self.validate()?;
        validate_ids(&[body_id.as_str(), part_id.as_str(), sign_id.as_str()])?;
        if body_id != &self.body_id {
            return Err(MembershipRefusal::WrongBody);
        }
        if expected_revision != self.revision {
            return Err(MembershipRefusal::StaleRevision);
        }
        if self.events.iter().any(|event| event.sign_id == *sign_id) {
            return Err(MembershipRefusal::DuplicateSign);
        }
        if self.events.len() == MAX_MEMBERSHIP_EVENTS {
            return Err(MembershipRefusal::EventCapacityExhausted);
        }
        Ok(())
    }

    fn next_revision(&self) -> Result<BodyMembershipRevision, MembershipRefusal> {
        self.revision
            .0
            .checked_add(1)
            .map(BodyMembershipRevision)
            .ok_or(MembershipRefusal::EventCapacityExhausted)
    }

    fn part_index(&self, part_id: &PartId) -> Result<usize, MembershipRefusal> {
        self.parts
            .iter()
            .position(|part| &part.part_id == part_id)
            .ok_or(MembershipRefusal::UnknownPart)
    }

    fn change_id(
        &self,
        part_id: &PartId,
        sign_id: &SignId,
        revision: BodyMembershipRevision,
    ) -> MembershipChangeId {
        MembershipChangeId::bound(bind_identity(
            "membership-change",
            &[self.body_id.as_str(), part_id.as_str(), sign_id.as_str()],
            revision.0,
        ))
    }

    fn push_event(&mut self, event: MembershipEvent) {
        self.events.push(event);
    }
}

fn replay_events(
    body_id: &BodyId,
    events: &[MembershipEvent],
) -> Result<Vec<PartMembership>, MembershipRefusal> {
    let mut parts: Vec<PartMembership> = Vec::new();
    for event in events {
        match &event.kind {
            MembershipEventKind::Admitted { proof_id } => {
                validate_ids(&[proof_id.as_str()])?;
                if parts.iter().any(|part| part.part_id == event.part_id)
                    || parts.len() == MAX_BODY_PARTS
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                parts.push(PartMembership {
                    part_id: event.part_id.clone(),
                    state: MembershipState::Admitted,
                    current: None,
                });
            }
            MembershipEventKind::HostAttached { observation } => {
                validate_observation(observation)?;
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted
                    || part.current.as_ref().is_some_and(|current| {
                        observation.sequence <= current.sequence
                            || (observation.host_id == current.host_id
                                && observation.boot_id == current.boot_id
                                && observation.offer_generation <= current.offer_generation)
                    })
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.current = Some(observation.clone());
            }
            MembershipEventKind::HostDetached { prior_boot_id } => {
                validate_ids(&[prior_boot_id.as_str()])?;
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted
                    || part.current.as_ref().map(|current| &current.boot_id) != Some(prior_boot_id)
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.current = None;
            }
            MembershipEventKind::Revoked => {
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.state = MembershipState::Revoked;
                part.current = None;
            }
        }
        if &event.body_id != body_id {
            return Err(MembershipRefusal::MalformedState);
        }
    }
    Ok(parts)
}

fn replay_part_mut<'a>(
    parts: &'a mut [PartMembership],
    part_id: &PartId,
) -> Result<&'a mut PartMembership, MembershipRefusal> {
    parts
        .iter_mut()
        .find(|part| &part.part_id == part_id)
        .ok_or(MembershipRefusal::MalformedState)
}

fn validate_observation(
    observation: &AuthenticatedHostObservation,
) -> Result<(), MembershipRefusal> {
    validate_ids(&[
        observation.host_id.as_str(),
        observation.boot_id.as_str(),
        observation.proof_id.as_str(),
    ])?;
    Ok(())
}
