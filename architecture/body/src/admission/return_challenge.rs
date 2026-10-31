//! Canonical return challenge issuance, bounded lookup and expiry retirement.
use super::*;

impl AdmissionManager {
    /// Recover only the exact still-valid challenge after an ambiguous transport
    /// result. This neither changes its lifetime nor resets signature attempts.
    /// The caller must use the issuing authority's unchanged monotonic clock;
    /// authority restart must first retire_unfinished_challenges durably.
    pub fn pending_return_challenge(
        &self,
        membership: &BodyMembership,
        part_id: &PartId,
        advertisement: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<Option<PartReturnChallenge>, AdmissionRefusal> {
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        let part = membership
            .parts
            .iter()
            .find(|part| &part.part_id == part_id)
            .ok_or(AdmissionRefusal::Membership(
                crate::MembershipRefusal::UnknownPart,
            ))?;
        if part.state != crate::MembershipState::Admitted || part.current.is_some() {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let key = self
            .continuity_keys
            .iter()
            .find(|key| &key.part_id == part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if key.host_id != advertisement.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        let Some(pending) = self
            .pending_returns
            .iter()
            .find(|p| &p.challenge.part_id == part_id)
        else {
            return Ok(None);
        };
        let c = &pending.challenge;
        if c.body_id != self.body_id || c.host_id != advertisement.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if c.boot_id != advertisement.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if c.offer_generation != advertisement.offer_generation {
            return Err(AdmissionRefusal::StaleOfferGeneration);
        }
        if now_millis < c.issued_at_millis || now_millis > c.expires_at_millis {
            return Err(AdmissionRefusal::Expired);
        }
        if pending.attempts >= MAX_ADMISSION_ATTEMPTS {
            return Err(AdmissionRefusal::AttemptsExhausted);
        }
        Ok(Some(c.clone()))
    }

    /// Retire one expired, non-exhausted return in the issuing authority's
    /// unchanged clock domain. The caller must durably checkpoint retirement
    /// before issuing/publishing another challenge. Unexpired and exhausted
    /// attempts cannot use this entrance to reset their signature attempt cap.
    pub fn retire_expired_return(
        &mut self,
        admission_id: &AdmissionId,
        now_millis: u64,
    ) -> Result<(), AdmissionRefusal> {
        let index = self
            .pending_returns
            .iter()
            .position(|p| &p.challenge.admission_id == admission_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let pending = &self.pending_returns[index];
        if pending.attempts >= MAX_ADMISSION_ATTEMPTS {
            return Err(AdmissionRefusal::AttemptsExhausted);
        }
        if now_millis <= pending.challenge.expires_at_millis {
            return Err(AdmissionRefusal::Replay);
        }
        self.pending_returns.remove(index);
        Ok(())
    }

    /// Retire only an expired exact-scope pending return; no unexpired or
    /// exhausted challenge is changed. Same authority-clock/durability custody
    /// obligations as retire_expired_return apply.
    pub fn retire_expired_return_for(
        &mut self,
        membership: &BodyMembership,
        part_id: &PartId,
        advertisement: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<(), AdmissionRefusal> {
        let pending = self
            .pending_returns
            .iter()
            .find(|p| &p.challenge.part_id == part_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let issued = pending.challenge.issued_at_millis;
        // Validate exact detached membership/Host/Boot/generation and attempt cap
        // at the original issue instant, without changing the challenge.
        let challenge = self
            .pending_return_challenge(membership, part_id, advertisement, issued)?
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        self.retire_expired_return(&challenge.admission_id, now_millis)
    }

    pub fn begin_return(
        &mut self,
        membership: &BodyMembership,
        part_id: &PartId,
        advertisement: &HostAdvertisement,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    ) -> Result<PartReturnChallenge, AdmissionRefusal> {
        self.validate_expiry(nonce, now_millis, expires_at_millis)?;
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if self.pending_returns.len() == MAX_PENDING_ADMISSIONS {
            return Err(AdmissionRefusal::PendingCapacityExhausted);
        }
        let part = membership
            .parts
            .iter()
            .find(|part| &part.part_id == part_id)
            .ok_or(AdmissionRefusal::Membership(
                crate::MembershipRefusal::UnknownPart,
            ))?;
        if part.state != crate::MembershipState::Admitted || part.current.is_some() {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let key = self
            .continuity_keys
            .iter()
            .find(|record| &record.part_id == part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if key.host_id != advertisement.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if self
            .pending_returns
            .iter()
            .any(|pending| pending.challenge.part_id == *part_id)
        {
            return Err(AdmissionRefusal::Replay);
        }
        let admission_id = AdmissionId::bound(crate::identity::bind_identity(
            "part-return",
            &[
                self.body_id.as_str(),
                part_id.as_str(),
                advertisement.host_id.as_str(),
                advertisement.boot_id.as_str(),
            ],
            now_millis,
        ));
        let challenge = PartReturnChallenge {
            admission_id,
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            nonce,
            issued_at_millis: now_millis,
            expires_at_millis,
        };
        self.pending_returns.push(PendingReturn {
            challenge: challenge.clone(),
            attempts: 0,
        });
        Ok(challenge)
    }
}
