use super::{sign, BodyLifecycleSession, BodyLifecycleSessionError};
#[cfg(feature = "authenticated-admission")]
use crate::{
    AdmissionManager, AdmissionSigns, MembershipCredential, SpawnAdmissionProof,
    SpawnInvitationClaim, SpawnInvitationSecret,
};
use crate::{BodyBiographyError, MembershipState};
#[cfg(feature = "authenticated-admission")]
use alloc::vec::Vec;
#[cfg(feature = "authenticated-admission")]
use conduit_core::HostAdvertisement;
use conduit_core::{BootId, HostId};

impl BodyLifecycleSession {
    #[cfg(feature = "authenticated-admission")]
    #[allow(clippy::too_many_arguments)]
    pub fn issue_invitation(
        &self,
        admissions: &mut AdmissionManager,
        secret: SpawnInvitationSecret,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<SpawnInvitationClaim, BodyLifecycleSessionError> {
        self.require_host(authority_host, authority_boot)?;
        admissions
            .issue_spawn_invitation(secret, nonce, now_millis, expires_at_millis)
            .map(|invitation| invitation.claim())
            .map_err(BodyLifecycleSessionError::Admission)
    }

    /// Complete one canonical single-use invitation at the body authority.
    /// Admission and authenticated presence are separate membership events;
    /// neither grants Plot or effect authority.
    #[cfg(feature = "authenticated-admission")]
    pub fn admit_invited_host(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &SpawnAdmissionProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, BodyLifecycleSessionError> {
        self.require_host(authority_host, authority_boot)?;
        self.make_membership_room(2)?;
        let first_sequence = self.next_sequence()?;
        let second_sequence = first_sequence
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
        let signs = AdmissionSigns {
            part_admitted: sign(authority_host, authority_boot, first_sequence),
            host_attached: sign(authority_host, authority_boot, second_sequence),
            candidate_admitted: sign(authority_host, authority_boot, second_sequence),
        };
        let mut membership = self.evidence.membership.clone();
        let mut next_admissions = admissions.clone();
        let prior_events = membership.events.len();
        let credential = next_admissions
            .complete_spawn(&mut membership, advertisement, proof, now_millis, signs)
            .map_err(BodyLifecycleSessionError::Admission)?;
        let events = membership.events[prior_events..]
            .iter()
            .zip([first_sequence, second_sequence])
            .map(|(event, sequence)| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 2 {
            return Err(BodyLifecycleSessionError::Biography(
                BodyBiographyError::InvalidEvidence,
            ));
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_membership_events(membership, &events)
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        *admissions = next_admissions;
        Ok(credential)
    }

    /// Record an observed carrier loss without revoking the admitted Part.
    /// The exact host/Boot ceases to be current, so its offers can no longer
    /// participate in planning; a future authenticated observation may attach
    /// the Part again under fresh truth.
    pub fn observe_host_lost(
        &mut self,
        lost_host: &HostId,
        lost_boot: &BootId,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_host(authority_host, authority_boot)?;
        let part_id = self
            .evidence
            .membership
            .parts
            .iter()
            .find(|part| {
                part.state == MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        &current.host_id == lost_host && &current.boot_id == lost_boot
                    })
            })
            .map(|part| part.part_id.clone())
            .ok_or(BodyLifecycleSessionError::StaleHost)?;
        self.make_membership_room(1)?;
        let sequence = self.next_sequence()?;
        let mut membership = self.evidence.membership.clone();
        let prior_events = membership.events.len();
        let change = membership
            .observe_offline(
                &self.evidence.body_id,
                membership.revision,
                &part_id,
                lost_boot,
                sign(authority_host, authority_boot, sequence),
            )
            .map_err(BodyLifecycleSessionError::Membership)?;
        if membership.events.len() != prior_events + 1 {
            return Err(BodyLifecycleSessionError::Biography(
                BodyBiographyError::InvalidEvidence,
            ));
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_membership_events(membership, &[(change, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }
}
