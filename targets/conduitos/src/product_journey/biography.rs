//! Retained canonical event evidence; ProductJourney remains the lifecycle owner.
use super::*;
use conduit_body::{BodyBiographyEvidence, BodyBiographyRecordKind, BodyLifecycleEvent};

impl ProductJourney {
    pub fn biography(&self) -> Option<&BodyBiographyEvidence> {
        self.biography.as_ref()
    }

    pub(super) fn retain_biography(&mut self) -> Result<(), JourneyError> {
        let Some(body) = &self.body else {
            return Ok(());
        };
        let evidence = self.prepare_biography(
            body,
            self.wake.as_ref(),
            self.membership.as_ref().ok_or(JourneyError::Membership)?,
        )?;
        if self.biography.as_ref() != Some(&evidence) {
            if let Some(kernel) = self.kernel.as_mut() {
                kernel
                    .refresh_tutorial(&evidence)
                    .map_err(JourneyError::Play)?;
            }
            self.biography = Some(evidence);
        }
        Ok(())
    }

    pub(super) fn prepare_biography(
        &self,
        body: &Body,
        wake: Option<&Wake>,
        membership: &BodyMembership,
    ) -> Result<BodyBiographyEvidence, JourneyError> {
        let mut evidence = self
            .biography
            .clone()
            .ok_or(JourneyError::InvalidTransition)?;
        let mut sequence = evidence
            .last_sequence()
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let membership_events: Vec<_> = membership
            .events
            .iter()
            .filter(|event| {
                !evidence.records.iter().any(|record| match &record.kind {
                    BodyBiographyRecordKind::PartAdmitted { change_id, .. }
                    | BodyBiographyRecordKind::HostJoined { change_id, .. }
                    | BodyBiographyRecordKind::HostLeft { change_id, .. }
                    | BodyBiographyRecordKind::PartRevoked { change_id, .. } => {
                        change_id == &event.change_id
                    }
                    _ => false,
                })
            })
            .map(|event| {
                let record = (event.change_id.clone(), sequence);
                sequence += 1;
                record
            })
            .collect();
        if !membership_events.is_empty() {
            evidence
                .append_membership_events(membership.clone(), &membership_events)
                .map_err(JourneyError::Biography)?;
        }
        let body_events: Vec<_> = body
            .events
            .iter()
            .skip(evidence.body.events.len())
            .filter(|event| {
                matches!(
                    event,
                    BodyLifecycleEvent::PlotAdmitted { .. }
                        | BodyLifecycleEvent::PlotRemoved { .. }
                        | BodyLifecycleEvent::Fulfilled { .. }
                )
            })
            .map(|event| {
                let record = (event.sign_id().clone(), sequence);
                sequence += 1;
                record
            })
            .collect();
        if !body_events.is_empty() {
            evidence
                .append_body_lifecycle_events(body.clone(), &body_events)
                .map_err(JourneyError::Biography)?;
        }
        if let Some(wake) = wake
            && (evidence
                .wakes
                .iter()
                .find(|prior| prior.wake_id == wake.wake_id)
                != Some(wake)
                || evidence.body != *body)
        {
            evidence
                .append_wake(body.clone(), wake.clone(), sequence)
                .map_err(JourneyError::Biography)?;
        }
        evidence.validate().map_err(JourneyError::Biography)?;
        Ok(evidence)
    }
}
