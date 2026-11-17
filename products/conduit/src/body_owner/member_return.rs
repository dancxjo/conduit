//! One finite signed member return, independent of a particular Mask.
//! Local control authenticates the caller; the continuity signature authenticates
//! the member. A route or public credential alone cannot attach a Part.
use super::{state, Owner};
use conduit_body::{
    AdmissionRefusal, BodyLifecycleSessionError, MembershipCredential, PartReturnChallenge,
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
    completed: Option<(PartReturnProof, MembershipCredential)>,
    completed_offer_digest: Option<[u8; 32]>,
    advertisement: HostAdvertisement,
    image_projection: Option<images::PendingImageReturn>,
    deadline: Instant,
    attempts: u8,
}

impl PendingMemberReturn {
    fn current_at(&self, now: Instant, at: u64) -> bool {
        now < self.deadline
            && at >= self.challenge.issued_at_millis
            && at < self.challenge.expires_at_millis
    }
}

impl Owner {
    pub(crate) fn begin_member_return(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
    ) -> Result<PartReturnChallenge, String> {
        let at = crate::durable_host::current_time_millis()?;
        self.begin_member_return_at(credential, advertisement, Instant::now(), at)
    }

    // Both clock observations belong to this request, including pending replay.
    pub(super) fn begin_member_return_at(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        now: Instant,
        at: u64,
    ) -> Result<PartReturnChallenge, String> {
        if self.host.is_playing() {
            return Err("body-play-active".into());
        }
        if self
            .pending_member_return
            .as_ref()
            .is_some_and(|pending| at < pending.challenge.issued_at_millis)
        {
            // Rollback cannot replace the pending challenge with a fresh lease.
            return Err("member return challenge expired".into());
        }
        if let Some(pending) = self
            .pending_member_return
            .as_ref()
            .filter(|pending| pending.current_at(now, at))
        {
            if pending.prior == *credential && pending.advertisement == *advertisement {
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
        if manager
            .receipts
            .iter()
            .rev()
            .find(|receipt| receipt.credential.part_id == credential.part_id)
            .is_none_or(|receipt| receipt.credential != *credential)
        {
            return Err("member return credential is stale or unknown".into());
        }
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        // Keep pending challenge custody ephemeral and bounded. Completion
        // recreates it against current biography before any durable mutation.
        let challenge = manager
            .begin_return(
                &self.session.evidence().membership,
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
            completed: None,
            completed_offer_digest: None,
            image_projection: None,
            attempts: 0,
            deadline: now + Duration::from_secs(10),
        });
        Ok(challenge)
    }

    pub(crate) fn complete_member_return(
        &mut self,
        root: &Path,
        proof: &PartReturnProof,
    ) -> Result<MembershipCredential, String> {
        let at = crate::durable_host::current_time_millis()?;
        self.complete_member_return_at(root, proof, Instant::now(), at)
    }

    pub(super) fn complete_member_return_at(
        &mut self,
        root: &Path,
        proof: &PartReturnProof,
        now: Instant,
        at: u64,
    ) -> Result<MembershipCredential, String> {
        if self.host.is_playing() {
            return Err("body-play-active".into());
        }
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return challenge absent")?;
        if !pending.current_at(now, at) {
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
        let mut next = self.session.clone();
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
                // Policy, capacity and lifecycle refusals do not attribute a
                // bad proof. Keep its original finite challenge and deadline.
                if matches!(
                    error,
                    BodyLifecycleSessionError::Admission(
                        AdmissionRefusal::WrongBody
                            | AdmissionRefusal::WrongHost
                            | AdmissionRefusal::StaleBoot
                            | AdmissionRefusal::StaleOfferGeneration
                            | AdmissionRefusal::StaleNonce
                            | AdmissionRefusal::Replay
                            | AdmissionRefusal::InvalidProof
                            | AdmissionRefusal::AttemptsExhausted
                    )
                ) {
                    if let Some(pending) = self.pending_member_return.as_mut() {
                        pending.attempts = pending.attempts.saturating_add(1);
                    }
                }
                return Err(format!("member return: {error:?}"));
            }
        };
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return pending custody lost")?;
        if !self
            .invited_mask_offers
            .contains_key(&pending.advertisement.host_id)
            && self.invited_mask_offers.len() >= conduit_body::MAX_BODY_PARTS
        {
            return Err("member return observed-offer pressure".into());
        }
        let advertisement_bytes =
            serde_json::to_vec(&pending.advertisement).map_err(|e| e.to_string())?;
        let digest = crate::durable_host::digest(&advertisement_bytes);
        let completed_offer_digest: [u8; 32] = {
            use sha2::{Digest, Sha256};
            Sha256::digest(&advertisement_bytes).into()
        };
        let observed =
            super::native_mask_route::InvitedMaskOffer::observed(&pending.advertisement, digest);
        let host_id = pending.advertisement.host_id.clone();
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
        self.invited_mask_offers.insert(host_id, observed);
        let pending = self
            .pending_member_return
            .as_mut()
            .ok_or("member return pending custody lost")?;
        pending.completed_offer_digest = Some(completed_offer_digest);
        pending.completed = Some((proof.clone(), credential.clone()));
        Ok(credential)
    }
}

#[path = "member_return/images.rs"]
mod images;
