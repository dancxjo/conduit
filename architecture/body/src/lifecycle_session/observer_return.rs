//! Membership-only return of an observer, preserving the current realization.
use super::{sign, BodyLifecycleSessionError as Error};
use crate::{
    AdmissionManager, AdmissionRefusal, BodyBiographyError, MembershipCredential, PartId,
    PartReturnChallenge, PartReturnProof,
};
use alloc::vec::Vec;
use conduit_core::{BootId, HostAdvertisement, HostId};

fn observer(advertisement: &HostAdvertisement) -> Result<(), Error> {
    if !advertisement.bases.is_empty()
        || !advertisement.resources.is_empty()
        || !advertisement.capabilities.is_empty()
        || !advertisement.planner_capabilities.is_empty()
    {
        return Err(Error::Admission(AdmissionRefusal::InvalidProof));
    }
    Ok(())
}
impl<P: crate::BodyPlanEvidence> super::BodyLifecycleOwner<P> {
    /// Issue the canonical continuity challenge for a detached observer Part.
    /// Caller must durably retain the updated manager before serving a challenge.
    /// No presence, offer eligibility, Plan, or Play is created by issuance.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_observer_return(
        &self,
        admissions: &mut AdmissionManager,
        part: &PartId,
        advertisement: &HostAdvertisement,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<PartReturnChallenge, Error> {
        self.require_host(authority_host, authority_boot)?;
        observer(advertisement)?;
        admissions
            .begin_return(
                &self.evidence.membership,
                part,
                advertisement,
                nonce,
                now_millis,
                expires_at_millis,
            )
            .map_err(Error::Admission)
    }

    /// Complete canonical observer continuity without changing the active local
    /// realization. Caller must checkpoint and physically read back membership
    /// and admissions before publishing any signed receipt or new presence.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_returning_observer(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.require_host(authority_host, authority_boot)?;
        observer(advertisement)?;
        let mut prepared = None;
        let archive = if super::has_membership_room(&self.evidence, 1) {
            None
        } else {
            let mut evidence = self.evidence.clone();
            let archive =
                super::prepare_membership_room(&mut evidence, self.pending_archives.len(), 1)?;
            prepared = Some(evidence);
            archive
        };
        let evidence = prepared.as_ref().unwrap_or(&self.evidence);
        let sequence = evidence
            .last_sequence()
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        let mut membership = evidence.membership.clone();
        let prior = membership.events.len();
        let mut manager = admissions.clone();
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
            .map(|e| (e.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 1 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        if let Some(mut evidence) = prepared {
            evidence
                .append_membership_events(membership, &events)
                .map_err(Error::Biography)?;
            self.evidence = evidence;
        } else {
            self.evidence
                .append_membership_events(membership, &events)
                .map_err(Error::Biography)?;
        }
        if let Some(archive) = archive {
            self.pending_archives.push(archive);
        }
        *admissions = manager;
        Ok(credential)
    }
}
