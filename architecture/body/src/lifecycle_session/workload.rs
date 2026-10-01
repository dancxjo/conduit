use super::{sign, BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{BodyState, ResidentForm};
use conduit_core::{BootId, HostId};

impl BodyLifecycleSession {
    /// Add checked meaning while Lulled. Current play replacement is a separate
    /// Host-orchestrated lifecycle; this cannot mutate an admitted plan.
    pub fn admit_form(
        &mut self,
        expected_revision: u64,
        form: ResidentForm,
        host: &HostId,
        boot: &BootId,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        if self.evidence.body.workload_revision != expected_revision {
            return Err(BodyLifecycleSessionError::StaleWorkload);
        }
        self.make_lifecycle_room(1, 0)?;
        let sequence = self.next_sequence()?;
        let sign_id = sign(host, boot, sequence);
        let body = self
            .evidence
            .body
            .admit_form(form, sign_id.clone())
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    /// Remove checked meaning only after the host has retired its actual Play.
    /// The body and membership survive even when the last Form is removed.
    pub fn remove_form(
        &mut self,
        expected_revision: u64,
        form: &ResidentForm,
        host: &HostId,
        boot: &BootId,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        if self.evidence.body.workload_revision != expected_revision {
            return Err(BodyLifecycleSessionError::StaleWorkload);
        }
        self.make_lifecycle_room(1, 0)?;
        let sequence = self.next_sequence()?;
        let sign_id = sign(host, boot, sequence);
        let body = self
            .evidence
            .body
            .remove_form(form, sign_id.clone())
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        if self.foreground.as_ref() == Some(form) {
            self.foreground = evidence.body.workset.forms().first().cloned();
        }
        self.evidence = evidence;
        Ok(())
    }
}
