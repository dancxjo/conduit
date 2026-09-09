use alloc::vec::Vec;
use conduit_body::{BodyBiographyEvidence, BodyState, MembershipState, WakeLifecycle};
use conduit_core::{BootId, HostId, bind_sign};

use crate::{WorkspaceBody, WorkspaceBodyError};

impl WorkspaceBody {
    /// Restore one local Body after the Host has acquired exclusive continuity
    /// ownership and observed that the old Boot is gone. This is a trusted Host
    /// report, not a grant or proof of hostile-code confinement. A missing Play
    /// is recorded as failed; its saved identity is never made current again.
    pub fn resume_here(
        mut evidence: BodyBiographyEvidence,
        host: &HostId,
        boot: &BootId,
    ) -> Result<Self, WorkspaceBodyError> {
        evidence.validate().map_err(WorkspaceBodyError::Biography)?;
        let mut parts = evidence
            .membership
            .parts
            .iter()
            .filter(|part| part.state == MembershipState::Admitted);
        let part = parts.next().ok_or(WorkspaceBodyError::StaleHost)?;
        if parts.next().is_some() {
            return Err(WorkspaceBodyError::StaleHost);
        }
        let prior = part.current.as_ref().ok_or(WorkspaceBodyError::StaleHost)?;
        if &prior.host_id != host || &prior.boot_id == boot {
            return Err(WorkspaceBodyError::StaleHost);
        }
        let part_id = part.part_id.clone();
        let mut observed = prior.clone();
        observed.boot_id = boot.clone();
        observed.sequence = observed
            .sequence
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)?;
        let mut sequence = evidence
            .records
            .last()
            .ok_or(WorkspaceBodyError::SequenceExhausted)?
            .sequence;
        let mut next = || {
            sequence = sequence
                .checked_add(1)
                .ok_or(WorkspaceBodyError::SequenceExhausted)?;
            Ok(sequence)
        };
        let mut membership = evidence.membership.clone();
        let mut changes = Vec::with_capacity(2);
        let at = next()?;
        let detached = membership
            .observe_offline(
                &evidence.body_id,
                membership.revision,
                &part_id,
                &prior.boot_id,
                bind_sign(host, boot, None, at).sign_id,
            )
            .map_err(|_| WorkspaceBodyError::StaleHost)?;
        changes.push((detached, at));
        let at = next()?;
        let attached = membership
            .observe_present(
                &evidence.body_id,
                membership.revision,
                &part_id,
                observed,
                bind_sign(host, boot, None, at).sign_id,
            )
            .map_err(|_| WorkspaceBodyError::StaleHost)?;
        changes.push((attached, at));
        evidence
            .append_membership_events(membership, &changes)
            .map_err(WorkspaceBodyError::Biography)?;
        if let BodyState::Awake { wake_id } = &evidence.body.state {
            let wake = evidence
                .wakes
                .iter()
                .find(|wake| &wake.wake_id == wake_id)
                .ok_or(WorkspaceBodyError::UnreconciledWake)?;
            let at = next()?;
            let failed = if matches!(
                wake.lifecycle,
                WakeLifecycle::Failed | WakeLifecycle::Lulled
            ) {
                wake.clone()
            } else {
                wake.fail(bind_sign(host, boot, None, at).sign_id)
                    .map_err(WorkspaceBodyError::Lifecycle)?
            };
            let body = evidence
                .body
                .retain_after_lull(&failed, bind_sign(host, boot, None, next()?).sign_id)
                .map_err(WorkspaceBodyError::Lifecycle)?;
            evidence
                .append_wake(body, failed, at)
                .map_err(WorkspaceBodyError::Biography)?;
        }
        Self::open(evidence)
    }
}
