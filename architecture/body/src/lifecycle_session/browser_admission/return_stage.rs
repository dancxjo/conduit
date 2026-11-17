//! Proposed return state, with ownership preserved until the caller persists it.
use super::*;
use alloc::borrow::Cow;

/// A proposal only: constructing this value neither persists nor adopts state,
/// issues authority, nor creates an acceptance receipt. The caller owns durable
/// retention before adopting its returned parts.
#[must_use = "staged return state must be retained and adopted explicitly"]
pub struct StagedMemberReturn {
    session: BodyLifecycleSession,
    admissions: AdmissionManager,
    credential: MembershipCredential,
}
impl StagedMemberReturn {
    pub fn into_parts(self) -> (BodyLifecycleSession, AdmissionManager, MembershipCredential) {
        (self.session, self.admissions, self.credential)
    }
}
impl BodyLifecycleSession {
    /// Stage the original returning-Host algorithm using an already owned
    /// admission manager. The original session stays unchanged on both success
    /// and refusal; this does not establish persistence or current authority.
    pub fn stage_returning_host(
        &self,
        admissions: AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority: (&HostId, &BootId),
    ) -> Result<StagedMemberReturn, Error> {
        self.stage_returning_host_custody(
            Cow::Owned(admissions),
            advertisement,
            proof,
            now_millis,
            authority,
        )
    }
    pub(super) fn stage_returning_host_custody(
        &self,
        admissions: Cow<'_, AdmissionManager>,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority: (&HostId, &BootId),
    ) -> Result<StagedMemberReturn, Error> {
        let (authority_host, authority_boot) = authority;
        self.require_lulled_admission(authority_host, authority_boot)?;
        let mut next = self.clone();
        next.make_membership_room(1)?;
        let sequence = next.next_sequence()?;
        let mut membership = next.evidence.membership.clone();
        let prior = membership.events.len();
        let mut manager = admissions.into_owned();
        let credential = manager
            .complete_return(
                &mut membership,
                advertisement,
                proof,
                now_millis,
                sign(authority_host, authority_boot, sequence),
            )
            .map_err(Error::Admission)?;
        let events = membership.events[prior..]
            .iter()
            .map(|event| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 1 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        next.evidence
            .append_membership_events(membership, &events)
            .map_err(Error::Biography)?;
        Ok(StagedMemberReturn {
            session: next,
            admissions: manager,
            credential,
        })
    }
}
