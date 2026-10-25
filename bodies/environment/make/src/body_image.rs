//! Birth the image's body on the construction host, using the ordinary Crèche
//! draft. The image carries intent and biography, never a future Boot or grant.
use conduit_birth_plot::{BirthDraft, BirthDraftRefusal, BirthPlotChoice};
use conduit_body::BodyBiographyEvidence;
use conduit_body_make::{PreparedBirthRefusal, PreparedBodyBirth};
use conduit_core::SignId;

pub struct PreparedBodyImage {
    evidence: BodyBiographyEvidence,
}

#[derive(Debug)]
pub enum BodyImageRefusal {
    Draft(BirthDraftRefusal),
    Birth(conduit_body::BodyLifecycleError),
    Membership(conduit_body::MembershipRefusal),
    Biography(conduit_body::BodyBiographyError),
}

impl PreparedBodyImage {
    /// Make supplies a retained construction UUID, attributable birth Sign and
    /// checked workload review. The existing Crèche selects its generated name
    /// and validates the exact initial workset; no naming policy is duplicated.
    pub fn prepare(
        construction_uuid: String,
        choices: Vec<BirthPlotChoice>,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, BodyImageRefusal> {
        use BodyImageRefusal as E;
        let draft = BirthDraft::new(construction_uuid, choices).map_err(E::Draft)?;
        let evidence = PreparedBodyBirth::from_reviewed_draft(
            &draft,
            draft.revision(),
            birth_sequence,
            birth_sign,
        )
        .map_err(|error| match error {
            PreparedBirthRefusal::Draft(error) => E::Draft(error),
            PreparedBirthRefusal::Birth(error) => E::Birth(error),
            PreparedBirthRefusal::Membership(error) => E::Membership(error),
            PreparedBirthRefusal::Biography(error) => E::Biography(error),
        })?
        .into_evidence();
        Ok(Self { evidence })
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }

    /// The caller can encode this ordinary biography into trusted image
    /// material. Boot must independently install/admit the local part before
    /// opening it as a current Host, planning, issuing invitations or starting.
    pub fn into_evidence(self) -> BodyBiographyEvidence {
        self.evidence
    }
}
