//! Import retained inert preparation through the same ordinary first-Host
//! admission as local Crèche birth. Later-Host media must await ordinary join.
use super::*;
use conduit_body::{PreparedBirthHostRole, PreparedBirthMedia};

impl ProductJourney {
    pub fn import_prepared_birth(
        &mut self,
        media: &PreparedBirthMedia,
        target: &str,
        profile_id: &str,
        build_id: &str,
    ) -> Result<(), JourneyError> {
        if self.body().is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        if self.status != JourneyStatus::World {
            return Err(JourneyError::InvalidTransition);
        }
        if media.binding().target != target
            || media.binding().profile_id != profile_id
            || media.binding().build_id != build_id
        {
            return Err(JourneyError::WrongTarget);
        }
        if media.role() != PreparedBirthHostRole::FirstHost {
            // This is an inert copy of existing Body birth evidence. It cannot
            // substitute for first-Host admission or rebirth on a later Host.
            return Err(JourneyError::AdmissionUnsupported);
        }
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        self.admit_initial_birth(media.evidence().clone())?;
        self.advance()
    }
}

#[cfg(test)]
#[path = "prepared_birth_tests.rs"]
mod tests;
