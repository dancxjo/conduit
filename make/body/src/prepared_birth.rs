//! One ordinary birth from the exact reviewed Crèche draft, before media make.
use conduit_birth_plot::{BirthDraft, BirthDraftRefusal};
use conduit_body::{
    Body, BodyBiographyError, BodyBiographyEvidence, BodyLifecycleError, BodyMembership,
    MembershipRefusal,
};
use conduit_core::SignId;

#[derive(Debug)]
pub enum PreparedBirthRefusal {
    Draft(BirthDraftRefusal),
    Birth(BodyLifecycleError),
    Membership(MembershipRefusal),
    Biography(BodyBiographyError),
}

/// Construction-time biography shared by all intended Host artifacts.
///
/// The owner supplies an attributable birth Sign and retains this value once.
/// It does not admit any Host, create a Boot, plan work, or grant effect authority.
/// Target builders consume the same evidence rather than repeating birth per Host.
pub struct PreparedBodyBirth {
    evidence: BodyBiographyEvidence,
}

impl PreparedBodyBirth {
    /// Reject an obsolete encounter before birth and preserve its edited name
    /// and exact selected workset. Naming policy remains owned by BirthDraft.
    pub fn from_reviewed_draft(
        draft: &BirthDraft,
        reviewed_revision: u32,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, PreparedBirthRefusal> {
        use PreparedBirthRefusal as E;
        let selected = draft.selection(reviewed_revision).map_err(E::Draft)?;
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

    pub fn into_evidence(self) -> BodyBiographyEvidence {
        self.evidence
    }
}
