//! Cold history transfer before a new realization exists. Every returned
//! segment remains owned until the caller checkpoints the segment and suffix.
use super::*;

impl<P: BodyPlanEvidence> BodyLifecycleOwner<P> {
    /// Stage one settled historical Wake for a complete durable checkpoint.
    /// This does not finish a live Play or acknowledge/discard an archive.
    /// The candidate copy is cold preparation only: it preserves the original
    /// evidence and backing on every refusal from the existing canonical sealer.
    pub fn seal_terminal_wake_for_checkpoint(&mut self) -> Result<bool, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        if !self.evidence.wakes.iter().any(|wake| {
            matches!(
                wake.lifecycle,
                crate::WakeLifecycle::Lulled | crate::WakeLifecycle::Failed
            )
        }) {
            return Ok(false);
        }
        if self.pending_archives.len() >= crate::MAX_BODY_BIOGRAPHY_WAKES {
            return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
        }
        self.pending_archives
            .try_reserve_exact(1)
            .map_err(|_| BodyLifecycleSessionError::ArchivePersistenceRequired)?;
        let mut candidate = self.evidence.clone();
        let segment = candidate
            .seal_oldest_terminal_wake()
            .map_err(BodyLifecycleSessionError::Biography)?;
        let Some(segment) = segment else {
            return Ok(false);
        };
        // Retiring history must also retire its now-empty resident capacity.
        // All this storage is dormant; canonical bytes and archive custody stay exact.
        candidate.wakes.shrink_to_fit();
        candidate.records.shrink_to_fit();
        candidate.body.events.shrink_to_fit();
        candidate.body.sign_ids.shrink_to_fit();
        self.evidence = candidate;
        self.pending_archives.push(segment);
        Ok(true)
    }
}
