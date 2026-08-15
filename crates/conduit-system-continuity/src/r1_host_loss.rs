//! Exact selected-Host loss consequence for one active immutable Plan.

use conduit_body::{HostPresenceEventKind, HostPresenceState, HostPresenceTable, PartId};
use conduit_core::{ControlLoopEvent, PlayUnsatisfiedReason, SignId};

use crate::{R1NewPlanRecovery, R1RecoveryError};

impl R1NewPlanRecovery {
    /// Applies one canonical unavailable presence observation to the active
    /// realization. This does not request planning, select a replacement, or
    /// alter the immutable Plan.
    pub fn observe_required_host_unavailable(
        &mut self,
        presence: &HostPresenceTable,
        part_id: &PartId,
        unsatisfied_sign_id: SignId,
    ) -> Result<(), R1RecoveryError> {
        presence.validate()?;
        if presence.body_id != self.body.body_id || self.plan_b.is_some() {
            return Err(R1RecoveryError::InvalidObservation);
        }
        let lease = presence
            .leases
            .iter()
            .find(|lease| &lease.part_id == part_id)
            .ok_or(R1RecoveryError::InvalidObservation)?;
        let loss = presence
            .events
            .iter()
            .rev()
            .find(|event| &event.part_id == part_id)
            .ok_or(R1RecoveryError::InvalidObservation)?;
        if lease.state != HostPresenceState::Unavailable
            || !matches!(
                loss.kind,
                HostPresenceEventKind::SessionLost | HostPresenceEventKind::Expired
            )
        {
            return Err(R1RecoveryError::InvalidObservation);
        }
        let unavailable = ControlLoopEvent::HostBecameUnavailable {
            plan_id: self.plan_a.plan_id.clone(),
            host_id: lease.host_id.clone(),
            boot_id: lease.boot_id.clone(),
            offer_generation: lease.offer_generation,
            observation_sign_id: loss.sign_id.clone(),
        };
        unavailable
            .validate_host_event(&self.plan_a)
            .map_err(|_| R1RecoveryError::WrongRealizationSubject)?;
        let unsatisfied = ControlLoopEvent::PlayBecameUnsatisfied {
            plan_id: self.plan_a.plan_id.clone(),
            reason: PlayUnsatisfiedReason::RequiredHostUnavailable,
            sign_id: unsatisfied_sign_id.clone(),
        };
        unsatisfied
            .validate()
            .map_err(|_| R1RecoveryError::InvalidObservation)?;

        self.reserve_event_slots(2)?;
        let next_wake = self
            .wake
            .became_unsatisfied(&self.plan_a.plan_id, unsatisfied_sign_id)?;
        self.push_event(unavailable)?;
        self.push_event(unsatisfied)?;
        self.wake = next_wake;
        Ok(())
    }
}
