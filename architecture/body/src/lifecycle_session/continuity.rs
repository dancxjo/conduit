use crate::{BodyBiographyEvidence, BodyState, MembershipState, WakeLifecycle};
use alloc::vec::Vec;
use conduit_core::{bind_sign, BootId, HostId};

use crate::{BodyLifecycleSession, BodyLifecycleSessionError};

impl BodyLifecycleSession {
    /// Restore one local body after the host has acquired exclusive continuity
    /// ownership and observed that the old Boot is gone. This is a trusted Host
    /// report, not a grant or proof of hostile-code confinement. A missing Play
    /// is recorded as failed; its saved identity is never made current again.
    pub fn resume_here(
        mut evidence: BodyBiographyEvidence,
        host: &HostId,
        boot: &BootId,
    ) -> Result<Self, BodyLifecycleSessionError> {
        evidence
            .validate()
            .map_err(BodyLifecycleSessionError::Biography)?;
        let mut parts = evidence.membership.parts.iter().filter(|part| {
            part.state == MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_some_and(|current| &current.host_id == host)
        });
        let part = parts.next().ok_or(BodyLifecycleSessionError::StaleHost)?;
        if parts.next().is_some() {
            return Err(BodyLifecycleSessionError::StaleHost);
        }
        let prior = part
            .current
            .as_ref()
            .ok_or(BodyLifecycleSessionError::StaleHost)?
            .clone();
        if &prior.host_id != host || &prior.boot_id == boot {
            return Err(BodyLifecycleSessionError::StaleHost);
        }
        let part_id = part.part_id.clone();
        let mut observed = prior.clone();
        observed.boot_id = boot.clone();
        observed.sequence = observed
            .sequence
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
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
            > crate::MAX_MEMBERSHIP_EVENTS
            || evidence.records.len().saturating_add(membership_changes)
                > crate::MAX_BODY_BIOGRAPHY_RECORDS
        {
            let segment = evidence
                .seal_membership_history()
                .map_err(BodyLifecycleSessionError::Biography)?
                .ok_or(BodyLifecycleSessionError::Biography(
                    crate::BodyBiographyError::CapacityExhausted,
                ))?;
            pending_archives.push(segment);
        }
        let mut sequence = evidence.last_sequence();
        let mut next = || {
            sequence = sequence
                .checked_add(1)
                .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
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
                .map_err(|_| BodyLifecycleSessionError::StaleHost)?;
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
            .map_err(|_| BodyLifecycleSessionError::StaleHost)?;
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
            .map_err(|_| BodyLifecycleSessionError::StaleHost)?;
        changes.push((attached, at));
        evidence
            .append_membership_events(membership, &changes)
            .map_err(BodyLifecycleSessionError::Biography)?;
        if let BodyState::Awake { wake_id } = &evidence.body.state {
            let wake = evidence
                .wakes
                .iter()
                .find(|wake| &wake.wake_id == wake_id)
                .ok_or(BodyLifecycleSessionError::UnreconciledWake)?;
            let at = next()?;
            let failed = if matches!(
                wake.lifecycle,
                WakeLifecycle::Failed | WakeLifecycle::Lulled
            ) {
                wake.clone()
            } else {
                wake.fail(bind_sign(host, boot, None, at).sign_id)
                    .map_err(BodyLifecycleSessionError::Lifecycle)?
            };
            let body = evidence
                .body
                .retain_after_lull(&failed, bind_sign(host, boot, None, next()?).sign_id)
                .map_err(BodyLifecycleSessionError::Lifecycle)?;
            evidence
                .append_wake(body, failed, at)
                .map_err(BodyLifecycleSessionError::Biography)?;
        }
        let mut body = Self::open(evidence)?;
        body.pending_archives = pending_archives;
        Ok(body)
    }
}
