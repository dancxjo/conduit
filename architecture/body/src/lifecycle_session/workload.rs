use super::{sign, BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{BodyState, ResidentPlot};
use conduit_core::{BootId, HostId};

impl BodyLifecycleSession {
    /// Add checked meaning while Lulled. Current play replacement is a separate
    /// Host-orchestrated lifecycle; this cannot mutate an admitted plan.
    pub fn admit_plot(
        &mut self,
        expected_revision: u64,
        plot: ResidentPlot,
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
            .admit_plot(plot, sign_id.clone())
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    /// Remove checked meaning only after the host has retired its actual Play.
    /// The body and membership survive even when the last Plot is removed.
    pub fn remove_plot(
        &mut self,
        expected_revision: u64,
        plot: &ResidentPlot,
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
            .remove_plot(plot, sign_id.clone())
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        if self.foreground.as_ref() == Some(plot) {
            self.foreground = evidence.body.workset.plots().first().cloned();
        }
        self.evidence = evidence;
        Ok(())
    }
}
