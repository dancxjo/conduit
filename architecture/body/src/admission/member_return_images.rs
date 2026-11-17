//! Retained-key attribution before resolving a returning member's offer images.
use crate::{
    AdmissionManager, AdmissionRefusal, BodyMembership, MemberReturnImagePresentation,
    MembershipRefusal, MembershipState,
};

impl AdmissionManager {
    /// Verify the canonical presentation using original admission key custody.
    /// This does not check latest-credential freshness, issue a challenge, admit
    /// the new Boot, resolve offers or grant authority. The owner must still use
    /// its existing begin/complete return checks, including pending replay rules.
    /// A previously signed credential can therefore pass attribution alone.
    pub fn verify_member_return_image_presentation(
        &self,
        membership: &BodyMembership,
        presentation: &MemberReturnImagePresentation,
    ) -> Result<(), AdmissionRefusal> {
        let credential = &presentation.credential;
        if membership.body_id != self.body_id || credential.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if credential.host_id != presentation.projection.context.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        let mut parts = membership
            .parts
            .iter()
            .filter(|part| part.part_id == credential.part_id);
        let part = parts
            .next()
            .ok_or(AdmissionRefusal::Membership(MembershipRefusal::UnknownPart))?;
        if parts.next().is_some() || part.state != MembershipState::Admitted {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let mut keys = self
            .continuity_keys
            .iter()
            .filter(|key| key.part_id == credential.part_id);
        let key = keys.next().ok_or(AdmissionRefusal::InvalidProof)?;
        if keys.next().is_some() {
            return Err(AdmissionRefusal::InvalidProof);
        }
        if key.host_id != credential.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        presentation
            .verify(&key.verifying_key)
            .map_err(|_| AdmissionRefusal::InvalidProof)
    }
}

#[cfg(test)]
#[path = "member_return_images/tests.rs"]
mod tests;
