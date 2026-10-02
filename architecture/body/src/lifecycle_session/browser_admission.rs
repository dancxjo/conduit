//! Browser challenge admission while lulled, without reopening lifecycle state.
use super::{sign, BodyLifecycleSession, BodyLifecycleSessionError as Error};
use crate::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyBiographyError, BodyState,
    CandidateInventory, MembershipCredential, PartReturnProof,
};
use alloc::vec::Vec;
use conduit_core::{BootId, HostAdvertisement, HostId};

impl BodyLifecycleSession {
    fn require_lulled_admission(&self, host: &HostId, boot: &BootId) -> Result<(), Error> {
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(Error::NotLulled);
        }
        Ok(())
    }

    /// Complete an explicitly authorized browser challenge atomically with the
    /// authoritative biography. Candidate discovery alone does not authorize this call.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_ambient_host(
        &mut self,
        admissions: &mut AdmissionManager,
        candidates: &mut CandidateInventory,
        proof: &AmbientAdmissionProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.require_lulled_admission(authority_host, authority_boot)?;
        let mut next = self.clone();
        next.make_membership_room(2)?;
        let first = next.next_sequence()?;
        let second = first.checked_add(1).ok_or(Error::SequenceExhausted)?;
        let mut membership = next.evidence.membership.clone();
        let prior = membership.events.len();
        let mut manager = admissions.clone();
        let mut inventory = candidates.clone();
        let credential = manager
            .complete_ambient(
                &mut inventory,
                &mut membership,
                proof,
                now_millis,
                AdmissionSigns {
                    part_admitted: sign(authority_host, authority_boot, first),
                    host_attached: sign(authority_host, authority_boot, second),
                    candidate_admitted: sign(authority_host, authority_boot, second),
                },
            )
            .map_err(Error::Admission)?;
        let events = membership.events[prior..]
            .iter()
            .zip([first, second])
            .map(|(event, sequence)| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 2 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        next.evidence
            .append_membership_events(membership, &events)
            .map_err(Error::Biography)?;
        *self = next;
        *admissions = manager;
        *candidates = inventory;
        Ok(credential)
    }

    /// Authenticate an admitted Part's return under its current Host and Boot;
    /// preserve all workset, foreground, and lifecycle state.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_returning_host(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.require_lulled_admission(authority_host, authority_boot)?;
        let mut next = self.clone();
        next.make_membership_room(1)?;
        let sequence = next.next_sequence()?;
        let mut membership = next.evidence.membership.clone();
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
            .map(|event| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 1 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        next.evidence
            .append_membership_events(membership, &events)
            .map_err(Error::Biography)?;
        *self = next;
        *admissions = manager;
        Ok(credential)
    }
}

#[cfg(test)]
mod tests;
