#[path = "return_challenge.rs"]
mod return_challenge;

use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, SignId};
use serde::{Deserialize, Serialize};

use super::{
    admission_transcript, verify_signature, AdmissionManager, AdmissionRefusal,
    ADMISSION_SIGNATURE_BYTES, MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_RECEIPTS,
    MAX_ADMISSION_TTL_MILLIS, MAX_PENDING_ADMISSIONS,
};
use crate::{
    AdmissionId, AuthenticatedHostObservation, BodyId, BodyMembership, MembershipCredential,
    MembershipCredentialId, MembershipProofId, PartId,
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
        self.validate_expiry_with_max(
            nonce,
            now_millis,
            expires_at_millis,
            MAX_ADMISSION_TTL_MILLIS,
        )
    }

    pub(super) fn validate_expiry_with_max(
        &self,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        maximum_ttl_millis: u64,
    ) -> Result<(), AdmissionRefusal> {
        if nonce == [0; 32] {
            return Err(AdmissionRefusal::StaleNonce);
        }
        if expires_at_millis <= now_millis || expires_at_millis - now_millis > maximum_ttl_millis {
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

    pub fn complete_return(
        &mut self,
        membership: &mut BodyMembership,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        attached_sign: SignId,
    ) -> Result<MembershipCredential, AdmissionRefusal> {
        let index = self
            .pending_returns
            .iter()
            .position(|pending| pending.challenge.admission_id == proof.admission_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let pending = self.pending_returns[index].clone();
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
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
        let credential = MembershipCredential {
            credential_id: MembershipCredentialId::bound(crate::identity::bind_identity(
                "membership-return-credential",
                &[
                    self.body_id.as_str(),
                    challenge.part_id.as_str(),
                    advertisement.host_id.as_str(),
                    advertisement.boot_id.as_str(),
                    challenge.admission_id.as_str(),
                ],
                now_millis,
            )),
            body_id: self.body_id.clone(),
            part_id: challenge.part_id.clone(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            issued_at_millis: now_millis,
        };
        self.pending_returns.remove(index);
        self.retain_receipt(challenge.admission_id.clone(), credential.clone())?;
        *membership = next;
        Ok(credential)
    }
}

/// Exact signature evidence from an already admitted, currently present Part.
/// This is not a grant, current execution permission, or a deserializable trust
/// record. Consumers must still bind their fresh nonce, purpose and endpoint to
/// `transcript` and recheck current membership before using the observation.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedAdmittedMessage {
    body_id: BodyId,
    part_id: PartId,
    host_id: HostId,
    boot_id: BootId,
    membership_revision: crate::BodyMembershipRevision,
    transcript: [u8; 32],
}
impl VerifiedAdmittedMessage {
    pub fn body_id(&self) -> &BodyId {
        &self.body_id
    }
    pub fn part_id(&self) -> &PartId {
        &self.part_id
    }
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub fn membership_revision(&self) -> crate::BodyMembershipRevision {
        self.membership_revision
    }
    pub fn transcript(&self) -> &[u8; 32] {
        &self.transcript
    }
}
impl AdmissionManager {
    /// Revoke every unfinished Boot-clock challenge when its issuing authority
    /// restarts. Continuity keys and completed receipts remain intact; old
    /// proofs cannot regain validity when the monotonic clock starts again.
    /// The caller must durably retain this change before accepting new proofs.
    pub fn retire_unfinished_challenges(&mut self) {
        self.pending.clear();
        self.invitations.clear();
        self.pending_returns.clear();
    }

    /// Authenticate only against the manager's retained canonical continuity
    /// key and the exact current admitted membership. IDs or a caller-supplied
    /// key cannot construct this evidence. This read-only operation leaves an
    /// existing local Wake/Plan/Play untouched, including during execution.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_admitted_message(
        &self,
        membership: &BodyMembership,
        part_id: &PartId,
        host_id: &HostId,
        boot_id: &BootId,
        transcript: &[u8; 32],
        signature: &[u8; ADMISSION_SIGNATURE_BYTES],
    ) -> Result<VerifiedAdmittedMessage, AdmissionRefusal> {
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        membership
            .validate()
            .map_err(AdmissionRefusal::Membership)?;
        let part = membership
            .parts
            .iter()
            .find(|part| &part.part_id == part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if part.state != crate::MembershipState::Admitted {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let current = part
            .current
            .as_ref()
            .ok_or(AdmissionRefusal::CandidateNotEligible)?;
        if &current.host_id != host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if &current.boot_id != boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        let key = self
            .continuity_keys
            .iter()
            .find(|record| &record.part_id == part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if &key.host_id != host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        verify_signature(key.verifying_key, transcript, *signature)?;
        Ok(VerifiedAdmittedMessage {
            body_id: self.body_id.clone(),
            part_id: part_id.clone(),
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            membership_revision: membership.revision,
            transcript: *transcript,
        })
    }
}

/// Signature evidence for read-only durable custody by the latest completed
/// credential of a still-admitted Part. Offline custody is not current presence,
/// a return, a fresh incarnation, or execution permission. Consumers must bind
/// their exact custody purpose/content to `transcript`; this witness has no
/// public constructor or deserialization entrance.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedRetainedCustodyMessage {
    credential: MembershipCredential,
    verifying_key: [u8; 32],
    membership_revision: crate::BodyMembershipRevision,
    transcript: [u8; 32],
}
impl VerifiedRetainedCustodyMessage {
    /// Public signer data from the actual retained continuity-key record.
    /// This conveys neither presence nor any additional custody authority.
    pub fn verifying_key(&self) -> &[u8; 32] {
        &self.verifying_key
    }
    pub fn credential(&self) -> &MembershipCredential {
        &self.credential
    }
    pub fn body_id(&self) -> &BodyId {
        &self.credential.body_id
    }
    pub fn part_id(&self) -> &PartId {
        &self.credential.part_id
    }
    pub fn host_id(&self) -> &HostId {
        &self.credential.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.credential.boot_id
    }
    pub fn membership_revision(&self) -> crate::BodyMembershipRevision {
        self.membership_revision
    }
    pub fn transcript(&self) -> &[u8; 32] {
        &self.transcript
    }
}
impl AdmissionManager {
    /// Observe the latest completed credential in this manager's bounded receipt
    /// log. This is data only: it proves neither current presence nor a signature.
    pub fn latest_completed_credential(&self, part_id: &PartId) -> Option<&MembershipCredential> {
        self.receipts
            .iter()
            .rev()
            .find(|receipt| &receipt.credential.part_id == part_id)
            .map(|receipt| &receipt.credential)
    }

    /// Authenticate a custody-only transcript using the retained continuity key
    /// and exact latest completed credential. A detached admitted Part may retain
    /// durable files while the producer restarts. Revocation, replacement and a
    /// later admitted Boot still fence its old credential. This operation never
    /// observes a Host as present or changes admission/return/lifecycle state.
    pub fn verify_retained_custody_message(
        &self,
        membership: &BodyMembership,
        credential: &MembershipCredential,
        transcript: &[u8; 32],
        signature: &[u8; ADMISSION_SIGNATURE_BYTES],
    ) -> Result<VerifiedRetainedCustodyMessage, AdmissionRefusal> {
        if membership.body_id != self.body_id || credential.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        membership
            .validate()
            .map_err(AdmissionRefusal::Membership)?;
        let part = membership
            .parts
            .iter()
            .find(|part| part.part_id == credential.part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if part.state != crate::MembershipState::Admitted {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let latest = self
            .latest_completed_credential(&credential.part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if latest.host_id != credential.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if latest.boot_id != credential.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if latest != credential {
            return Err(AdmissionRefusal::InvalidProof);
        }
        if let Some(current) = &part.current {
            if current.host_id != credential.host_id {
                return Err(AdmissionRefusal::WrongHost);
            }
            if current.boot_id != credential.boot_id {
                return Err(AdmissionRefusal::StaleBoot);
            }
        }
        let key = self
            .continuity_keys
            .iter()
            .find(|record| record.part_id == credential.part_id)
            .ok_or(AdmissionRefusal::InvalidProof)?;
        if key.host_id != credential.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        verify_signature(key.verifying_key, transcript, *signature)?;
        Ok(VerifiedRetainedCustodyMessage {
            credential: credential.clone(),
            verifying_key: key.verifying_key,
            membership_revision: membership.revision,
            transcript: *transcript,
        })
    }
}
