//! Exact current Crèche Face and typed actions for an ordinary native Mask.
use super::{Error, FrontDoor};
use conduit_birth_plot::{BirthActionOutcome, BirthFaceBasis, BirthFaceRefusal};
use conduit_presentation::{FaceInteraction, MaskShow, Presentation};

impl FrontDoor {
    pub fn creche_face(&self, basis: &BirthFaceBasis) -> Result<Presentation, Error> {
        if basis.host_id != self.host_id || basis.boot_id != self.boot_id {
            return Err(Error::StaleInput);
        }
        self.arrival
            .as_ref()
            .ok_or(Error::ActionUnavailable)?
            .draft
            .face(basis)
            .map_err(|_| Error::Presentation)
    }

    /// A birth selection still goes through ProductJourney's one lifecycle
    /// authority. Changed drafts advance the current Face revision first.
    pub fn apply_creche_face_interaction(
        &mut self,
        basis: &BirthFaceBasis,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<BirthActionOutcome, Error> {
        if basis.host_id != self.host_id || basis.boot_id != self.boot_id {
            return Err(Error::StaleInput);
        }
        if self
            .journey
            .as_ref()
            .is_some_and(|journey| journey.body_id.is_some())
        {
            return Err(Error::ActionUnavailable);
        }
        let outcome = self
            .arrival
            .as_mut()
            .ok_or(Error::ActionUnavailable)?
            .draft
            .apply_face_interaction(basis, show, input)
            .map_err(|refusal| match refusal {
                BirthFaceRefusal::Interaction(_) => Error::StaleInput,
                _ => Error::ActionRefused,
            })?;
        if matches!(outcome, BirthActionOutcome::Changed) {
            self.advance()?;
        }
        Ok(outcome)
    }
}
