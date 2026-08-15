use alloc::{string::String, vec::Vec};
use conduit_core::{BootId, HostId, LinkBindingId, OfferGeneration, SignId};
use serde::{Deserialize, Serialize};

use crate::{
    BodyId, BodyMembership, MembershipProofId, MembershipRefusal, MembershipState, PartId,
    MAX_BODY_PARTS, MAX_LIFECYCLE_ID_BYTES,
};

mod validation;

pub const MAX_PRESENCE_EVENTS: usize = 64;
pub const MAX_PRESENCE_LEASE_MILLIS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostPresenceClockScale {
    Milliseconds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPresenceClock {
    pub basis_id: String,
    pub scale: HostPresenceClockScale,
    pub resolution_ticks: u64,
    pub uncertainty_ticks: u64,
}

impl HostPresenceClock {
    pub fn new(
        basis_id: String,
        scale: HostPresenceClockScale,
        resolution_ticks: u64,
        uncertainty_ticks: u64,
    ) -> Result<Self, HostPresenceRefusal> {
        validate_identity(&basis_id)?;
        if resolution_ticks == 0 {
            return Err(HostPresenceRefusal::InvalidClock);
        }
        Ok(Self {
            basis_id,
            scale,
            resolution_ticks,
            uncertainty_ticks,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostPresenceState {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPresenceLease {
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub membership_proof_id: MembershipProofId,
    pub session_binding_id: LinkBindingId,
    pub sequence: u64,
    pub observed_at_millis: u64,
    pub expires_at_millis: u64,
    pub state: HostPresenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostPresenceEventKind {
    Started,
    Renewed,
    SessionLost,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPresenceEvent {
    pub revision: u64,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub membership_proof_id: MembershipProofId,
    pub session_binding_id: LinkBindingId,
    pub sequence: u64,
    pub observed_at_millis: u64,
    pub expires_at_millis: u64,
    pub sign_id: SignId,
    pub kind: HostPresenceEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPresenceTable {
    pub body_id: BodyId,
    pub clock: HostPresenceClock,
    pub maximum_lease_millis: u64,
    pub revision: u64,
    pub dropped_event_count: u64,
    pub leases: Vec<HostPresenceLease>,
    pub events: Vec<HostPresenceEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HostPresenceRefusal {
    EmptyIdentity,
    IdentityTooLong,
    WrongBody,
    UnknownPart,
    RevokedPart,
    HostUnavailable,
    WrongHost,
    StaleBoot,
    StaleOfferGeneration,
    StaleMembershipProof,
    WrongSession,
    StaleSequence,
    ClockRegressed,
    InvalidClock,
    LeaseDurationZero,
    LeaseDurationTooLong,
    LeaseDeadlineOverflow,
    LeaseStillCurrent,
    PresenceCapacityExhausted,
    RevisionOverflow,
    MalformedState,
    Membership(MembershipRefusal),
}

impl HostPresenceTable {
    pub fn new(
        body_id: BodyId,
        clock: HostPresenceClock,
        maximum_lease_millis: u64,
    ) -> Result<Self, HostPresenceRefusal> {
        validate_identity(body_id.as_str())?;
        validate_identity(&clock.basis_id)?;
        if clock.resolution_ticks == 0 {
            return Err(HostPresenceRefusal::InvalidClock);
        }
        if maximum_lease_millis == 0 {
            return Err(HostPresenceRefusal::LeaseDurationZero);
        }
        if maximum_lease_millis > MAX_PRESENCE_LEASE_MILLIS {
            return Err(HostPresenceRefusal::LeaseDurationTooLong);
        }
        Ok(Self {
            body_id,
            clock,
            maximum_lease_millis,
            revision: 0,
            dropped_event_count: 0,
            leases: Vec::new(),
            events: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        membership: &BodyMembership,
        part_id: &PartId,
        session_binding_id: LinkBindingId,
        sequence: u64,
        observed_at_millis: u64,
        lease_millis: u64,
        sign_id: SignId,
    ) -> Result<(), HostPresenceRefusal> {
        self.validate()?;
        let current = self.current_membership_host(membership, part_id)?;
        validate_identity(session_binding_id.as_str())?;
        validate_identity(sign_id.as_str())?;
        if sequence == 0 {
            return Err(HostPresenceRefusal::StaleSequence);
        }
        let expires_at_millis = self.deadline(observed_at_millis, lease_millis)?;
        let (event_revision, dropped_event_count) = self.next_event_state()?;
        if let Some(index) = self.lease_index(part_id) {
            let prior = &self.leases[index];
            if prior.state == HostPresenceState::Available {
                return Err(HostPresenceRefusal::LeaseStillCurrent);
            }
            if sequence <= prior.sequence {
                return Err(HostPresenceRefusal::StaleSequence);
            }
            if observed_at_millis < prior.observed_at_millis {
                return Err(HostPresenceRefusal::ClockRegressed);
            }
            self.leases[index] = HostPresenceLease {
                part_id: part_id.clone(),
                host_id: current.host_id.clone(),
                boot_id: current.boot_id.clone(),
                offer_generation: current.offer_generation,
                membership_proof_id: current.proof_id.clone(),
                session_binding_id: session_binding_id.clone(),
                sequence,
                observed_at_millis,
                expires_at_millis,
                state: HostPresenceState::Available,
            };
        } else {
            if self.leases.len() == MAX_BODY_PARTS {
                return Err(HostPresenceRefusal::PresenceCapacityExhausted);
            }
            self.leases.push(HostPresenceLease {
                part_id: part_id.clone(),
                host_id: current.host_id.clone(),
                boot_id: current.boot_id.clone(),
                offer_generation: current.offer_generation,
                membership_proof_id: current.proof_id.clone(),
                session_binding_id: session_binding_id.clone(),
                sequence,
                observed_at_millis,
                expires_at_millis,
                state: HostPresenceState::Available,
            });
        }
        self.commit_event(
            HostPresenceEvent {
                revision: event_revision,
                part_id: part_id.clone(),
                host_id: current.host_id.clone(),
                boot_id: current.boot_id.clone(),
                offer_generation: current.offer_generation,
                membership_proof_id: current.proof_id.clone(),
                session_binding_id,
                sequence,
                observed_at_millis,
                expires_at_millis,
                sign_id,
                kind: HostPresenceEventKind::Started,
            },
            dropped_event_count,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn renew(
        &mut self,
        membership: &BodyMembership,
        part_id: &PartId,
        session_binding_id: &LinkBindingId,
        sequence: u64,
        observed_at_millis: u64,
        lease_millis: u64,
        sign_id: SignId,
    ) -> Result<(), HostPresenceRefusal> {
        self.validate()?;
        let current = self.current_membership_host(membership, part_id)?;
        validate_identity(session_binding_id.as_str())?;
        validate_identity(sign_id.as_str())?;
        let expires_at_millis = self.deadline(observed_at_millis, lease_millis)?;
        let (event_revision, dropped_event_count) = self.next_event_state()?;
        let index = self
            .lease_index(part_id)
            .ok_or(HostPresenceRefusal::HostUnavailable)?;
        let lease = &mut self.leases[index];
        if lease.state != HostPresenceState::Available {
            return Err(HostPresenceRefusal::HostUnavailable);
        }
        if lease.host_id != current.host_id {
            return Err(HostPresenceRefusal::WrongHost);
        }
        if lease.boot_id != current.boot_id {
            return Err(HostPresenceRefusal::StaleBoot);
        }
        if lease.offer_generation != current.offer_generation {
            return Err(HostPresenceRefusal::StaleOfferGeneration);
        }
        if lease.membership_proof_id != current.proof_id {
            return Err(HostPresenceRefusal::StaleMembershipProof);
        }
        if lease.session_binding_id != *session_binding_id {
            return Err(HostPresenceRefusal::WrongSession);
        }
        if sequence <= lease.sequence {
            return Err(HostPresenceRefusal::StaleSequence);
        }
        if observed_at_millis < lease.observed_at_millis {
            return Err(HostPresenceRefusal::ClockRegressed);
        }
        lease.sequence = sequence;
        lease.observed_at_millis = observed_at_millis;
        lease.expires_at_millis = expires_at_millis;
        let event = HostPresenceEvent {
            revision: event_revision,
            part_id: part_id.clone(),
            host_id: lease.host_id.clone(),
            boot_id: lease.boot_id.clone(),
            offer_generation: lease.offer_generation,
            membership_proof_id: lease.membership_proof_id.clone(),
            session_binding_id: session_binding_id.clone(),
            sequence,
            observed_at_millis,
            expires_at_millis,
            sign_id,
            kind: HostPresenceEventKind::Renewed,
        };
        self.commit_event(event, dropped_event_count);
        Ok(())
    }

    pub fn expire(
        &mut self,
        membership: &mut BodyMembership,
        part_id: &PartId,
        observed_at_millis: u64,
        sign_id: SignId,
    ) -> Result<(), HostPresenceRefusal> {
        self.validate()?;
        validate_identity(sign_id.as_str())?;
        if membership.body_id != self.body_id {
            return Err(HostPresenceRefusal::WrongBody);
        }
        let index = self
            .lease_index(part_id)
            .ok_or(HostPresenceRefusal::HostUnavailable)?;
        let lease = &self.leases[index];
        if lease.state != HostPresenceState::Available {
            return Err(HostPresenceRefusal::HostUnavailable);
        }
        if observed_at_millis < lease.observed_at_millis {
            return Err(HostPresenceRefusal::ClockRegressed);
        }
        if observed_at_millis < lease.expires_at_millis {
            return Err(HostPresenceRefusal::LeaseStillCurrent);
        }
        let (event_revision, dropped_event_count) = self.next_event_state()?;
        let event = HostPresenceEvent {
            revision: event_revision,
            part_id: part_id.clone(),
            host_id: lease.host_id.clone(),
            boot_id: lease.boot_id.clone(),
            offer_generation: lease.offer_generation,
            membership_proof_id: lease.membership_proof_id.clone(),
            session_binding_id: lease.session_binding_id.clone(),
            sequence: lease.sequence,
            observed_at_millis,
            expires_at_millis: lease.expires_at_millis,
            sign_id: sign_id.clone(),
            kind: HostPresenceEventKind::Expired,
        };
        membership
            .observe_offline(
                &self.body_id,
                membership.revision,
                part_id,
                &lease.boot_id,
                sign_id,
            )
            .map_err(HostPresenceRefusal::Membership)?;
        self.leases[index].state = HostPresenceState::Unavailable;
        self.commit_event(event, dropped_event_count);
        Ok(())
    }

    pub fn lose_session(
        &mut self,
        membership: &mut BodyMembership,
        part_id: &PartId,
        session_binding_id: &LinkBindingId,
        observed_at_millis: u64,
        sign_id: SignId,
    ) -> Result<(), HostPresenceRefusal> {
        self.validate()?;
        validate_identity(session_binding_id.as_str())?;
        validate_identity(sign_id.as_str())?;
        if membership.body_id != self.body_id {
            return Err(HostPresenceRefusal::WrongBody);
        }
        let index = self
            .lease_index(part_id)
            .ok_or(HostPresenceRefusal::HostUnavailable)?;
        let lease = &self.leases[index];
        if lease.state != HostPresenceState::Available {
            return Err(HostPresenceRefusal::HostUnavailable);
        }
        if lease.session_binding_id != *session_binding_id {
            return Err(HostPresenceRefusal::WrongSession);
        }
        if observed_at_millis < lease.observed_at_millis {
            return Err(HostPresenceRefusal::ClockRegressed);
        }
        let (event_revision, dropped_event_count) = self.next_event_state()?;
        let event = HostPresenceEvent {
            revision: event_revision,
            part_id: part_id.clone(),
            host_id: lease.host_id.clone(),
            boot_id: lease.boot_id.clone(),
            offer_generation: lease.offer_generation,
            membership_proof_id: lease.membership_proof_id.clone(),
            session_binding_id: lease.session_binding_id.clone(),
            sequence: lease.sequence,
            observed_at_millis,
            expires_at_millis: lease.expires_at_millis,
            sign_id: sign_id.clone(),
            kind: HostPresenceEventKind::SessionLost,
        };
        membership
            .observe_offline(
                &self.body_id,
                membership.revision,
                part_id,
                &lease.boot_id,
                sign_id,
            )
            .map_err(HostPresenceRefusal::Membership)?;
        self.leases[index].state = HostPresenceState::Unavailable;
        self.commit_event(event, dropped_event_count);
        Ok(())
    }

    fn current_membership_host<'a>(
        &self,
        membership: &'a BodyMembership,
        part_id: &PartId,
    ) -> Result<&'a crate::AuthenticatedHostObservation, HostPresenceRefusal> {
        membership
            .validate()
            .map_err(HostPresenceRefusal::Membership)?;
        if membership.body_id != self.body_id {
            return Err(HostPresenceRefusal::WrongBody);
        }
        let part = membership
            .parts
            .iter()
            .find(|part| &part.part_id == part_id)
            .ok_or(HostPresenceRefusal::UnknownPart)?;
        if part.state != MembershipState::Admitted {
            return Err(HostPresenceRefusal::RevokedPart);
        }
        part.current
            .as_ref()
            .ok_or(HostPresenceRefusal::HostUnavailable)
    }

    fn deadline(
        &self,
        observed_at_millis: u64,
        lease_millis: u64,
    ) -> Result<u64, HostPresenceRefusal> {
        if lease_millis == 0 {
            return Err(HostPresenceRefusal::LeaseDurationZero);
        }
        if lease_millis > self.maximum_lease_millis {
            return Err(HostPresenceRefusal::LeaseDurationTooLong);
        }
        observed_at_millis
            .checked_add(lease_millis)
            .ok_or(HostPresenceRefusal::LeaseDeadlineOverflow)
    }

    fn lease_index(&self, part_id: &PartId) -> Option<usize> {
        self.leases
            .iter()
            .position(|lease| &lease.part_id == part_id)
    }

    fn next_event_state(&self) -> Result<(u64, u64), HostPresenceRefusal> {
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(HostPresenceRefusal::RevisionOverflow)?;
        let dropped_event_count = if self.events.len() == MAX_PRESENCE_EVENTS {
            self.dropped_event_count
                .checked_add(1)
                .ok_or(HostPresenceRefusal::RevisionOverflow)?
        } else {
            self.dropped_event_count
        };
        Ok((revision, dropped_event_count))
    }

    fn commit_event(&mut self, event: HostPresenceEvent, dropped_event_count: u64) {
        self.revision = event.revision;
        if self.events.len() == MAX_PRESENCE_EVENTS {
            self.events.remove(0);
        }
        self.dropped_event_count = dropped_event_count;
        self.events.push(event);
    }
}

fn validate_identity(value: &str) -> Result<(), HostPresenceRefusal> {
    if value.is_empty() {
        return Err(HostPresenceRefusal::EmptyIdentity);
    }
    if value.len() > MAX_LIFECYCLE_ID_BYTES {
        return Err(HostPresenceRefusal::IdentityTooLong);
    }
    Ok(())
}
