//! Birth the image's body on the construction host, using the ordinary Crèche
//! draft. The image carries intent and biography, never a future Boot or grant.
use conduit_birth_plot::{BirthDraft, BirthDraftRefusal, BirthPlotChoice};
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership};
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
        let selected = draft.selection(draft.revision()).map_err(E::Draft)?;
        let body = Body::born_with_plots(selected.workset, birth_sequence, birth_sign)
            .map_err(E::Birth)?;
        let membership = BodyMembership::new(body.body_id.clone()).map_err(E::Membership)?;
        let evidence = BodyBiographyEvidence::born(body, membership, selected.friendly_name)
            .map_err(E::Biography)?;
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
