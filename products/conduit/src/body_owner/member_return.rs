//! One finite signed member return, independent of a particular Mask.
//! Local control authenticates the caller; the continuity signature authenticates
//! the member. A route or public credential alone cannot attach a Part.
use super::{state, Owner};
use conduit_body::{
    AuthenticatedHostObservation, BodyLifecycleSession, MembershipCredential, PartReturnChallenge,
    PartReturnProof,
};
use conduit_core::HostAdvertisement;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) struct PendingMemberReturn {
    challenge: PartReturnChallenge,
    prior: MembershipCredential,
    prior_attachment: Option<AuthenticatedHostObservation>,
    completed: Option<(PartReturnProof, MembershipCredential)>,
    advertisement: HostAdvertisement,
    deadline: Instant,
    attempts: u8,
    presentation: Option<conduit_body::MemberReturnImagePresentation>,
}

impl Owner {
    /// Conditional replacement is preparation only. The real attachment stays
    /// current until its continuity key proves the exact new Boot and the
    /// combined retirement/return commits. Transport loss alone is not leave.
    fn member_return_preparation(
        &self,
        prior: &MembershipCredential,
        advertisement: &HostAdvertisement,
    ) -> Result<(BodyLifecycleSession, Option<AuthenticatedHostObservation>), String> {
        let latest = self.admissions.as_ref().and_then(|manager| {
            manager
                .receipts
                .iter()
                .rev()
                .find(|receipt| receipt.credential.part_id == prior.part_id)
        });
        if latest.is_none_or(|receipt| receipt.credential != *prior) {
            return Err("member return credential is stale or unknown".into());
        }
        let attachment = self
            .session
            .evidence()
            .membership
            .parts
            .iter()
            .find(|part| part.part_id == prior.part_id)
            .ok_or("member return Part absent")?
            .current
            .clone();
        let mut next = self.session.clone();
        if let Some(current) = &attachment {
            if current.host_id != prior.host_id
                || current.boot_id != prior.boot_id
                || advertisement.host_id != prior.host_id
                || advertisement.boot_id == current.boot_id
            {
                return Err("member return does not replace its prior Host Boot".into());
            }
            let authority = self.host.advertisement();
            next.observe_host_lost(
                &current.host_id,
                &current.boot_id,
                &authority.host_id,
                &authority.boot_id,
            )
            .map_err(|error| format!("member return preparation: {error:?}"))?;
        }
        Ok((next, attachment))
    }

    /// Release only this completed return slot after its signed durable receipt
    /// has been delivered. Retirement cannot attach a member or create offers.
    #[cfg(target_os = "linux")]
    pub(crate) fn retire_completed_selected_return(
        &mut self,
        credential: &MembershipCredential,
    ) -> Result<(), String> {
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return slot absent")?;
        if pending
            .completed
            .as_ref()
            .is_none_or(|(_, accepted)| accepted != credential)
        {
            return Err("cannot retire an incomplete or different member return".into());
        }
        self.pending_member_return = None;
        Ok(())
    }

    pub(crate) fn begin_member_return(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
    ) -> Result<PartReturnChallenge, String> {
        self.begin_member_image_return(credential, advertisement, None)
    }

    /// The control boundary independently resolves images before entering here.
    pub(crate) fn begin_member_image_return(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        presentation: Option<conduit_body::MemberReturnImagePresentation>,
    ) -> Result<PartReturnChallenge, String> {
        if let Some(claim) = &presentation {
            let mut context = advertisement.clone();
            context.capabilities.clear();
            if claim.credential != *credential || claim.projection.context != context {
                return Err("member return presentation differs from pending context".into());
            }
            claim
                .projection
                .validate()
                .map_err(|e| format!("member grants: {e:?}"))?;
        }
        if self.host.is_playing() {
            return Err("body-play-active".into());
        }
        if let Some(pending) = self
            .pending_member_return
            .as_ref()
            .filter(|p| Instant::now() < p.deadline)
        {
            if pending.prior == *credential
                && pending.advertisement == *advertisement
                && pending.presentation == presentation
            {
                if let Some((_, current)) = &pending.completed {
                    self.require_current_native_member(
                        &super::super::MemberPresentationBasis::new(
                            current.clone(),
                            advertisement.clone(),
                        )?,
                    )?;
                }
                return Ok(pending.challenge.clone());
            }
            return Err("member return challenge already pending".into());
        }
        if credential.body_id != self.session.evidence().body_id
            || credential.host_id != advertisement.host_id
            || credential.host_id == self.host.advertisement().host_id
        {
            return Err("member return differs from retained Body and Host".into());
        }
        let mut manager = self
            .admissions
            .clone()
            .ok_or("member return has no admission custody")?;
        let (basis, prior_attachment) =
            self.member_return_preparation(credential, advertisement)?;
        let at = crate::durable_host::current_time_millis()?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        // Keep pending challenge custody ephemeral and bounded. Completion
        // recreates it against current biography before any durable mutation.
        let challenge = manager
            .begin_return(
                &basis.evidence().membership,
                &credential.part_id,
                advertisement,
                nonce,
                at,
                at.checked_add(10_000)
                    .ok_or("member return deadline overflow")?,
            )
            .map_err(|e| format!("member return: {e:?}"))?;
        self.pending_member_return = Some(PendingMemberReturn {
            challenge: challenge.clone(),
            advertisement: advertisement.clone(),
            prior: credential.clone(),
            prior_attachment,
            completed: None,
            attempts: 0,
            presentation,
            deadline: Instant::now() + Duration::from_secs(10),
        });
        Ok(challenge)
    }

    pub(crate) fn complete_member_return(
        &mut self,
        root: &Path,
        proof: &PartReturnProof,
    ) -> Result<MembershipCredential, String> {
        if self.host.is_playing() {
            return Err("body-play-active".into());
        }
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return challenge absent")?;
        let at = crate::durable_host::current_time_millis()?;
        if Instant::now() >= pending.deadline
            || at < pending.challenge.issued_at_millis
            || at >= pending.challenge.expires_at_millis
        {
            return Err("member return challenge expired".into());
        }
        if proof.admission_id != pending.challenge.admission_id {
            return Err("member return differs from pending challenge".into());
        }
        if let Some((accepted, credential)) = &pending.completed {
            let latest = self.admissions.as_ref().and_then(|m| {
                m.receipts
                    .iter()
                    .rev()
                    .find(|r| r.credential.part_id == credential.part_id)
            });
            if accepted == proof && latest.is_some_and(|r| r.credential == *credential) {
                self.require_current_native_member(&super::super::MemberPresentationBasis::new(
                    credential.clone(),
                    pending.advertisement.clone(),
                )?)?;
                return Ok(credential.clone());
            }
            return Err("member return replay differs from committed proof".into());
        }
        if pending.attempts >= conduit_body::MAX_ADMISSION_ATTEMPTS {
            return Err("member return proof attempts exhausted".into());
        }
        self.retain_admission_capacity(root)?;
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return pending custody lost")?;
        let mut manager = self
            .admissions
            .clone()
            .ok_or("member return admission custody absent")?;
        let (mut next, attachment) =
            self.member_return_preparation(&pending.prior, &pending.advertisement)?;
        if attachment != pending.prior_attachment {
            return Err("member return prior attachment changed".into());
        }
        let challenge = manager
            .begin_return(
                &next.evidence().membership,
                &pending.challenge.part_id,
                &pending.advertisement,
                pending.challenge.nonce,
                pending.challenge.issued_at_millis,
                pending.challenge.expires_at_millis,
            )
            .map_err(|e| format!("member return: {e:?}"))?;
        if challenge != pending.challenge {
            return Err("member return challenge changed".into());
        }
        let authority = self.host.advertisement();
        let result = next.admit_returning_host(
            &mut manager,
            &pending.advertisement,
            proof,
            at,
            &authority.host_id,
            &authority.boot_id,
        );
        let credential = match result {
            Ok(credential) => credential,
            Err(error) => {
                if let Some(pending) = self.pending_member_return.as_mut() {
                    pending.attempts = pending.attempts.saturating_add(1);
                }
                return Err(format!("member return: {error:?}"));
            }
        };
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return pending custody lost")?;
        let grants = if let Some(presentation) = &pending.presentation {
            let witness = manager
                .current_member_continuity(
                    &next.evidence().membership,
                    &credential.part_id,
                    &pending.advertisement,
                )
                .map_err(|e| format!("member return continuity: {e:?}"))?;
            presentation
                .verify(&witness.peer().verifying_key)
                .map_err(|e| format!("member return grant signature: {e:?}"))?;
            presentation.projection.authority_grants.clone()
        } else {
            Vec::new()
        };
        let offers = self.next_member_offers_with_grants(
            next.evidence(),
            pending.advertisement.clone(),
            grants,
        )?;
        let prior = pending.prior.clone();
        state::retain_session(
            root,
            &mut next,
            self.last_execution.as_ref(),
            Some(&manager),
            None,
            None,
        )?;
        self.session = next;
        self.admissions = Some(manager);
        self.current_host_offers = offers;
        self.retire_native_member_route(&prior);
        self.pending_member_return
            .as_mut()
            .ok_or("member return pending custody lost")?
            .completed = Some((proof.clone(), credential.clone()));
        Ok(credential)
    }
}
