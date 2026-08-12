use alloc::vec::Vec;
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, SignId};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::{bind_identity, validate_ids};
use crate::{
    AdmissionId, AuthenticatedHostObservation, BodyId, BodyLifecycleError, BodyMembership,
    CandidateId, CandidateInventory, CandidateRefusal, CandidateState, MembershipCredentialId,
    MembershipProofId, MembershipRefusal, PartId,
};

mod invitation;

use invitation::InvitationRecord;
pub use invitation::{SpawnAdmissionProof, SpawnInvitation, SpawnInvitationSecret};

pub const MAX_PENDING_ADMISSIONS: usize = 16;
pub const MAX_ADMISSION_RECEIPTS: usize = 32;
pub const MAX_SPAWN_INVITATIONS: usize = 16;
pub const MAX_ADMISSION_ATTEMPTS: u8 = 3;
pub const MAX_ADMISSION_TTL_MILLIS: u64 = 60_000;
pub const ADMISSION_SIGNATURE_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionChallenge {
    pub admission_id: AdmissionId,
    pub body_id: BodyId,
    pub candidate_id: CandidateId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

impl AdmissionChallenge {
    pub fn signing_transcript(&self) -> [u8; 32] {
        admission_transcript(
            "ambient-admission-v1",
            &self.body_id,
            self.admission_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
            self.offer_generation,
            &self.nonce,
            self.expires_at_millis,
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AmbientAdmissionProof {
    pub admission_id: AdmissionId,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub nonce: [u8; 32],
    pub signature: [u8; ADMISSION_SIGNATURE_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionSigns {
    pub part_admitted: SignId,
    pub host_attached: SignId,
    pub candidate_admitted: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipCredential {
    pub credential_id: MembershipCredentialId,
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub issued_at_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PendingAdmission {
    challenge: AdmissionChallenge,
    verifying_key: [u8; 32],
    attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionReceipt {
    pub admission_id: AdmissionId,
    pub credential: MembershipCredential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionManager {
    pub body_id: BodyId,
    pending: Vec<PendingAdmission>,
    invitations: Vec<InvitationRecord>,
    pub receipts: Vec<AdmissionReceipt>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AdmissionRefusal {
    EmptyIdentity,
    IdentityTooLong,
    WrongBody,
    WrongHost,
    StaleBoot,
    StaleOfferGeneration,
    StaleNonce,
    Expired,
    Replay,
    InvalidProof,
    AttemptsExhausted,
    WeakSecret,
    InvalidExpiry,
    UnknownAdmission,
    UnknownInvitation,
    CandidateNotEligible,
    PendingCapacityExhausted,
    InvitationCapacityExhausted,
    ReceiptCapacityExhausted,
    Candidate(CandidateRefusal),
    Membership(MembershipRefusal),
}

impl From<BodyLifecycleError> for AdmissionRefusal {
    fn from(value: BodyLifecycleError) -> Self {
        match value {
            BodyLifecycleError::EmptyIdentity => Self::EmptyIdentity,
            BodyLifecycleError::IdentityTooLong => Self::IdentityTooLong,
            _ => Self::InvalidProof,
        }
    }
}

impl From<CandidateRefusal> for AdmissionRefusal {
    fn from(value: CandidateRefusal) -> Self {
        Self::Candidate(value)
    }
}

impl From<MembershipRefusal> for AdmissionRefusal {
    fn from(value: MembershipRefusal) -> Self {
        Self::Membership(value)
    }
}

impl AdmissionManager {
    pub fn new(body_id: BodyId) -> Result<Self, AdmissionRefusal> {
        validate_ids(&[body_id.as_str()])?;
        Ok(Self {
            body_id,
            pending: Vec::new(),
            invitations: Vec::new(),
            receipts: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_ambient(
        &mut self,
        candidates: &mut CandidateInventory,
        candidate_id: &CandidateId,
        verifying_key: [u8; 32],
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        requesting_sign: SignId,
    ) -> Result<AdmissionChallenge, AdmissionRefusal> {
        self.validate_expiry(nonce, now_millis, expires_at_millis)?;
        if candidates.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if self.pending.len() == MAX_PENDING_ADMISSIONS {
            return Err(AdmissionRefusal::PendingCapacityExhausted);
        }
        let candidate = candidates
            .candidates
            .iter()
            .find(|candidate| &candidate.candidate_id == candidate_id)
            .ok_or(AdmissionRefusal::CandidateNotEligible)?;
        if candidate.state != CandidateState::Discovered {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        VerifyingKey::from_bytes(&verifying_key).map_err(|_| AdmissionRefusal::InvalidProof)?;
        let advertisement = &candidate.observation.advertisement;
        let admission_id = AdmissionId::bound(bind_identity(
            "ambient-admission",
            &[
                self.body_id.as_str(),
                candidate_id.as_str(),
                advertisement.host_id.as_str(),
                advertisement.boot_id.as_str(),
            ],
            now_millis,
        ));
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.admission_id == admission_id)
        {
            return Err(AdmissionRefusal::Replay);
        }
        let challenge = AdmissionChallenge {
            admission_id,
            body_id: self.body_id.clone(),
            candidate_id: candidate_id.clone(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            nonce,
            issued_at_millis: now_millis,
            expires_at_millis,
        };
        let mut next_candidates = candidates.clone();
        next_candidates.transition(
            candidate_id,
            CandidateState::RequestingAdmission,
            requesting_sign,
        )?;
        self.pending.push(PendingAdmission {
            challenge: challenge.clone(),
            verifying_key,
            attempts: 0,
        });
        *candidates = next_candidates;
        Ok(challenge)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_ambient(
        &mut self,
        candidates: &mut CandidateInventory,
        membership: &mut BodyMembership,
        proof: &AmbientAdmissionProof,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<MembershipCredential, AdmissionRefusal> {
        if self
            .receipts
            .iter()
            .any(|receipt| receipt.admission_id == proof.admission_id)
        {
            return Err(AdmissionRefusal::Replay);
        }
        let index = self
            .pending
            .iter()
            .position(|pending| pending.challenge.admission_id == proof.admission_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let pending = self.pending[index].clone();
        if pending.attempts >= MAX_ADMISSION_ATTEMPTS {
            return Err(AdmissionRefusal::AttemptsExhausted);
        }
        self.validate_ambient_proof(&pending, proof, candidates, now_millis)?;
        verify_signature(
            pending.verifying_key,
            &pending.challenge.signing_transcript(),
            proof.signature,
        )
        .map_err(|refusal| {
            self.pending[index].attempts = self.pending[index].attempts.saturating_add(1);
            if self.pending[index].attempts >= MAX_ADMISSION_ATTEMPTS {
                AdmissionRefusal::AttemptsExhausted
            } else {
                refusal
            }
        })?;
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        let advertisement = candidates
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == pending.challenge.candidate_id)
            .ok_or(AdmissionRefusal::CandidateNotEligible)?
            .observation
            .advertisement
            .clone();
        let mut next_membership = membership.clone();
        let credential = self.attach(
            &mut next_membership,
            &advertisement,
            now_millis,
            &pending.challenge.admission_id,
            &signs,
        )?;
        let mut next_candidates = candidates.clone();
        next_candidates.transition(
            &pending.challenge.candidate_id,
            CandidateState::Admitted,
            signs.candidate_admitted,
        )?;
        self.retain_receipt(pending.challenge.admission_id.clone(), credential.clone())?;
        self.pending.remove(index);
        *membership = next_membership;
        *candidates = next_candidates;
        Ok(credential)
    }

    pub fn disconnect_ambient(
        &mut self,
        candidates: &mut CandidateInventory,
        admission_id: &AdmissionId,
        lost_sign: SignId,
    ) -> Result<(), AdmissionRefusal> {
        let index = self
            .pending
            .iter()
            .position(|pending| &pending.challenge.admission_id == admission_id)
            .ok_or(AdmissionRefusal::UnknownAdmission)?;
        let candidate_id = self.pending[index].challenge.candidate_id.clone();
        let mut next = candidates.clone();
        next.transition(&candidate_id, CandidateState::Lost, lost_sign)?;
        self.pending.remove(index);
        *candidates = next;
        Ok(())
    }

    fn validate_ambient_proof(
        &self,
        pending: &PendingAdmission,
        proof: &AmbientAdmissionProof,
        candidates: &CandidateInventory,
        now_millis: u64,
    ) -> Result<(), AdmissionRefusal> {
        let challenge = &pending.challenge;
        if proof.body_id != challenge.body_id || proof.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if proof.host_id != challenge.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if proof.boot_id != challenge.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if proof.nonce != challenge.nonce {
            return Err(AdmissionRefusal::StaleNonce);
        }
        if now_millis > challenge.expires_at_millis {
            return Err(AdmissionRefusal::Expired);
        }
        let candidate = candidates
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == challenge.candidate_id)
            .ok_or(AdmissionRefusal::CandidateNotEligible)?;
        if candidate.state != CandidateState::RequestingAdmission {
            return Err(AdmissionRefusal::CandidateNotEligible);
        }
        let advertisement = &candidate.observation.advertisement;
        if advertisement.host_id != challenge.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if advertisement.boot_id != challenge.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if advertisement.offer_generation != challenge.offer_generation {
            return Err(AdmissionRefusal::StaleOfferGeneration);
        }
        Ok(())
    }

    fn attach(
        &self,
        membership: &mut BodyMembership,
        advertisement: &HostAdvertisement,
        now_millis: u64,
        admission_id: &AdmissionId,
        signs: &AdmissionSigns,
    ) -> Result<MembershipCredential, AdmissionRefusal> {
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        let part_id = PartId::bind(
            &self.body_id,
            admission_id.as_str(),
            self.receipts.len() as u64,
        )?;
        let credential_id = MembershipCredentialId::bound(bind_identity(
            "membership-credential",
            &[
                self.body_id.as_str(),
                part_id.as_str(),
                advertisement.host_id.as_str(),
                advertisement.boot_id.as_str(),
                admission_id.as_str(),
            ],
            now_millis,
        ));
        let proof_id = MembershipProofId::bind(credential_id.as_str())?;
        let mut next = membership.clone();
        next.admit(
            &self.body_id,
            next.revision,
            part_id.clone(),
            proof_id.clone(),
            signs.part_admitted.clone(),
        )?;
        next.observe_present(
            &self.body_id,
            next.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                proof_id,
                sequence: 0,
            },
            signs.host_attached.clone(),
        )?;
        *membership = next;
        Ok(MembershipCredential {
            credential_id,
            body_id: self.body_id.clone(),
            part_id,
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            issued_at_millis: now_millis,
        })
    }

    fn retain_receipt(
        &mut self,
        admission_id: AdmissionId,
        credential: MembershipCredential,
    ) -> Result<(), AdmissionRefusal> {
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        self.receipts.push(AdmissionReceipt {
            admission_id,
            credential,
        });
        Ok(())
    }

    fn validate_expiry(
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
}

fn verify_signature(
    verifying_key: [u8; 32],
    transcript: &[u8; 32],
    signature: [u8; ADMISSION_SIGNATURE_BYTES],
) -> Result<(), AdmissionRefusal> {
    let key =
        VerifyingKey::from_bytes(&verifying_key).map_err(|_| AdmissionRefusal::InvalidProof)?;
    key.verify(transcript, &Signature::from_bytes(&signature))
        .map_err(|_| AdmissionRefusal::InvalidProof)
}

#[allow(clippy::too_many_arguments)]
fn admission_transcript(
    domain: &str,
    body_id: &BodyId,
    protocol_id: &str,
    host_id: &str,
    boot_id: &str,
    offer_generation: OfferGeneration,
    nonce: &[u8; 32],
    expires_at_millis: u64,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    for value in [domain, body_id.as_str(), protocol_id, host_id, boot_id] {
        digest.update((value.len() as u32).to_le_bytes());
        digest.update(value.as_bytes());
    }
    digest.update(offer_generation.0.to_le_bytes());
    digest.update(nonce);
    digest.update(expires_at_millis.to_le_bytes());
    digest.finalize().into()
}
