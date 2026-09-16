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
        let mut parts = evidence.membership.parts.iter().filter(|part| {
            part.state == MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_some_and(|current| &current.host_id == host)
        });
        let part = parts.next().ok_or(WorkspaceBodyError::StaleHost)?;
        if parts.next().is_some() {
            return Err(WorkspaceBodyError::StaleHost);
        }
        let prior = part
            .current
            .as_ref()
            .ok_or(WorkspaceBodyError::StaleHost)?
            .clone();
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
        let membership_changes = evidence
            .membership
            .parts
            .iter()
            .filter(|part| {
                part.current
                    .as_ref()
                    .is_some_and(|current| &current.host_id != host)
            })
            .count()
            .saturating_add(2);
        let mut pending_archives = Vec::new();
        if evidence
            .membership
            .events
            .len()
            .saturating_add(membership_changes)
            > conduit_body::MAX_MEMBERSHIP_EVENTS
            || evidence.records.len().saturating_add(membership_changes)
                > conduit_body::MAX_BODY_BIOGRAPHY_RECORDS
        {
            let segment = evidence
                .seal_membership_history()
                .map_err(WorkspaceBodyError::Biography)?
                .ok_or(WorkspaceBodyError::Biography(
                    conduit_body::BodyBiographyError::CapacityExhausted,
                ))?;
            pending_archives.push(segment);
        }
        let mut sequence = evidence.last_sequence();
        let mut next = || {
            sequence = sequence
                .checked_add(1)
                .ok_or(WorkspaceBodyError::SequenceExhausted)?;
            Ok(sequence)
        };
        let mut membership = evidence.membership.clone();
        let mut changes = Vec::with_capacity(membership.parts.len() + 1);
        let remote = membership
            .parts
            .iter()
            .filter_map(|part| {
                part.current
                    .as_ref()
                    .filter(|current| &current.host_id != host)
                    .map(|current| (part.part_id.clone(), current.boot_id.clone()))
            })
            .collect::<Vec<_>>();
        for (remote_part, remote_boot) in remote {
            let at = next()?;
            let detached = membership
                .observe_offline(
                    &evidence.body_id,
                    membership.revision,
                    &remote_part,
                    &remote_boot,
                    bind_sign(host, boot, None, at).sign_id,
                )
                .map_err(|_| WorkspaceBodyError::StaleHost)?;
            changes.push((detached, at));
        }
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
        let mut body = Self::open(evidence)?;
        body.pending_archives = pending_archives;
        Ok(body)
    }
}
