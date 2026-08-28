use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, SignId};
use serde::{Deserialize, Serialize};

use super::{
    admission_transcript, verify_signature, AdmissionManager, AdmissionRefusal,
    ADMISSION_SIGNATURE_BYTES, MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_TTL_MILLIS,
    MAX_PENDING_ADMISSIONS,
};
use crate::{
    AdmissionId, AuthenticatedHostObservation, BodyId, BodyMembership, MembershipProofId, PartId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ContinuityKeyRecord {
    pub(super) part_id: PartId,
    pub(super) host_id: HostId,
    pub(super) verifying_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PendingReturn {
    pub(super) challenge: PartReturnChallenge,
    pub(super) attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartReturnChallenge {
    pub admission_id: AdmissionId,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

impl PartReturnChallenge {
    pub fn signing_transcript(&self) -> [u8; 32] {
        admission_transcript(
            "part-return-v1",
            &self.body_id,
            self.part_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
            self.offer_generation,
            &self.nonce,
            self.expires_at_millis,
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PartReturnProof {
    pub admission_id: AdmissionId,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub nonce: [u8; 32],
    pub signature: [u8; ADMISSION_SIGNATURE_BYTES],
}

impl AdmissionManager {
    pub(super) fn validate_expiry(
        &self,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    ) -> Result<(), AdmissionRefusal> {
        if nonce == [0; 32] {
            return Err(AdmissionRefusal::StaleNonce);
        }
        if expires_at_millis <= now_millis
            || expires_at_millis - now_millis > MAX_ADMISSION_TTL_MILLIS
        {
            return Err(AdmissionRefusal::InvalidExpiry);
        }
        Ok(())
    }

    pub(super) fn ensure_continuity_capacity(&self) -> Result<(), AdmissionRefusal> {
        if self.continuity_keys.len() == crate::MAX_BODY_PARTS {
            return Err(AdmissionRefusal::Membership(
                crate::MembershipRefusal::PartCapacityExhausted,
            ));
        }
        Ok(())
    }

    pub(super) fn retain_continuity_key(
        &mut self,
        part_id: PartId,
        host_id: HostId,
        verifying_key: [u8; 32],
    ) {
        self.continuity_keys.push(ContinuityKeyRecord {
            part_id,
            host_id,
            verifying_key,
        });
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

    pub fn complete_return(
        &mut self,
        membership: &mut BodyMembership,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        attached_sign: SignId,
    ) -> Result<(), AdmissionRefusal> {
        let index = self
            .pending_returns
            .iter()
            .position(|pending| pending.challenge.admission_id == proof.admission_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let pending = self.pending_returns[index].clone();
        if pending.attempts >= MAX_ADMISSION_ATTEMPTS {
            return Err(AdmissionRefusal::AttemptsExhausted);
        }
        let challenge = &pending.challenge;
        if proof.body_id != self.body_id || proof.body_id != challenge.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if proof.part_id != challenge.part_id {
            return Err(AdmissionRefusal::InvalidProof);
        }
        if proof.host_id != challenge.host_id || advertisement.host_id != challenge.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if proof.boot_id != challenge.boot_id || advertisement.boot_id != challenge.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if proof.nonce != challenge.nonce {
            return Err(AdmissionRefusal::StaleNonce);
        }
        if advertisement.offer_generation != challenge.offer_generation {
            return Err(AdmissionRefusal::StaleOfferGeneration);
        }
        if now_millis > challenge.expires_at_millis
            || challenge.expires_at_millis - challenge.issued_at_millis > MAX_ADMISSION_TTL_MILLIS
        {
            return Err(AdmissionRefusal::Expired);
        }
        let key = self
            .continuity_keys
            .iter()
            .find(|record| record.part_id == challenge.part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        verify_signature(
            key.verifying_key,
            &challenge.signing_transcript(),
            proof.signature,
        )
        .map_err(|refusal| {
            self.pending_returns[index].attempts =
                self.pending_returns[index].attempts.saturating_add(1);
            if self.pending_returns[index].attempts >= MAX_ADMISSION_ATTEMPTS {
                AdmissionRefusal::AttemptsExhausted
            } else {
                refusal
            }
        })?;
        let proof_id = MembershipProofId::bind(proof.admission_id.as_str())?;
        let mut next = membership.clone();
        next.observe_present(
            &self.body_id,
            next.revision,
            &challenge.part_id,
            AuthenticatedHostObservation {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                proof_id,
                sequence: next.revision.0 + 1,
            },
            attached_sign,
        )?;
        self.pending_returns.remove(index);
        *membership = next;
        Ok(())
    }
}
