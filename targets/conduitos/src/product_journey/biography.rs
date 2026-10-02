//! Read-only projections of the canonical lifecycle session evidence.
use super::*;
use conduit_body::BodyBiographyEvidence;

impl ProductJourney {
    pub fn biography(&self) -> Option<&BodyBiographyEvidence> {
        self.session.as_ref().map(BodyLifecycleSession::evidence)
    }

    pub(super) fn refresh_tutorial(&mut self) -> Result<(), JourneyError> {
        if let (Some(session), Some(kernel)) = (&self.session, &mut self.kernel) {
            kernel
                .refresh_tutorial(session.evidence())
                .map_err(JourneyError::Play)?;
        }
        Ok(())
    }

    pub(super) fn require_archive_storage(
        session: &BodyLifecycleSession,
    ) -> Result<(), JourneyError> {
        if session.pending_archives().is_empty() {
            Ok(())
        } else {
            Err(JourneyError::Lifecycle(
                BodyLifecycleSessionError::ArchivePersistenceRequired,
            ))
        }
    }
}
