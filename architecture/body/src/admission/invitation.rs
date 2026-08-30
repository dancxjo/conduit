use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use super::{
    admission_transcript, verify_signature, AdmissionManager, AdmissionRefusal, AdmissionSigns,
    MembershipCredential, ADMISSION_SIGNATURE_BYTES, MAX_ADMISSION_ATTEMPTS,
    MAX_ADMISSION_RECEIPTS, MAX_SPAWN_INVITATIONS,
};
use crate::identity::bind_identity;
use crate::{AdmissionId, BodyId, BodyMembership, SpawnInvitationId};

#[derive(Clone, PartialEq, Eq)]
pub struct SpawnInvitationSecret(pub(super) [u8; 32]);

impl core::fmt::Debug for SpawnInvitationSecret {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("SpawnInvitationSecret([REDACTED])")
    }
}

impl SpawnInvitationSecret {
    pub fn from_csprng_bytes(bytes: [u8; 32]) -> Result<Self, AdmissionRefusal> {
        if bytes == [0; 32] {
            return Err(AdmissionRefusal::WeakSecret);
        }
        Ok(Self(bytes))
    }

    pub fn sign(&self, transcript: &[u8; 32]) -> [u8; ADMISSION_SIGNATURE_BYTES] {
        use ed25519_dalek::Signer;
        SigningKey::from_bytes(&self.0).sign(transcript).to_bytes()
    }

    /// Copies the secret into an exact target-provisioning buffer.
    ///
    /// This is only for an admitted local deployment mechanism that must place
    /// a self-joining invitation into a fresh spore. Callers must zero the
    /// returned buffer immediately after transfer and must never log it.
    pub fn copy_for_target_provisioning(&self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnInvitation {
    pub invitation_id: SpawnInvitationId,
    pub body_id: BodyId,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
    pub secret: SpawnInvitationSecret,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnInvitationClaim {
    pub invitation_id: SpawnInvitationId,
    pub body_id: BodyId,
    pub nonce: [u8; 32],
    pub expires_at_millis: u64,
}

impl SpawnInvitationClaim {
    pub fn signing_transcript(
        &self,
        host_id: &HostId,
        boot_id: &BootId,
        offer_generation: OfferGeneration,
    ) -> [u8; 32] {
        admission_transcript(
            "spawn-invitation-v1",
            &self.body_id,
            self.invitation_id.as_str(),
            host_id.as_str(),
            boot_id.as_str(),
            offer_generation,
            &self.nonce,
            self.expires_at_millis,
        )
    }
}

impl SpawnInvitation {
    pub fn claim(&self) -> SpawnInvitationClaim {
        SpawnInvitationClaim {
            invitation_id: self.invitation_id.clone(),
            body_id: self.body_id.clone(),
            nonce: self.nonce,
            expires_at_millis: self.expires_at_millis,
        }
    }

    pub fn signing_transcript(
        &self,
        host_id: &HostId,
        boot_id: &BootId,
        offer_generation: OfferGeneration,
    ) -> [u8; 32] {
        self.claim()
            .signing_transcript(host_id, boot_id, offer_generation)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SpawnAdmissionProof {
    pub invitation_id: SpawnInvitationId,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub nonce: [u8; 32],
    pub signature: [u8; ADMISSION_SIGNATURE_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InvitationRecord {
    pub(super) invitation_id: SpawnInvitationId,
    pub(super) verifying_key: [u8; 32],
    pub(super) nonce: [u8; 32],
    pub(super) issued_at_millis: u64,
    pub(super) expires_at_millis: u64,
    pub(super) used: bool,
    pub(super) attempts: u8,
}

impl AdmissionManager {
    pub fn issue_spawn_invitation(
        &mut self,
        secret: SpawnInvitationSecret,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    ) -> Result<SpawnInvitation, AdmissionRefusal> {
        self.validate_expiry(nonce, now_millis, expires_at_millis)?;
        if self.invitations.len() == MAX_SPAWN_INVITATIONS {
            return Err(AdmissionRefusal::InvitationCapacityExhausted);
        }
        let verifying_key = SigningKey::from_bytes(&secret.0).verifying_key().to_bytes();
        let sequence = now_millis
            .checked_mul((MAX_SPAWN_INVITATIONS + 1) as u64)
            .and_then(|base| base.checked_add(self.invitations.len() as u64))
            .ok_or(AdmissionRefusal::InvalidExpiry)?;
        let invitation_id = SpawnInvitationId::bound(bind_identity(
            "spawn-invitation",
            &[self.body_id.as_str()],
            sequence,
        ));
        self.invitations.push(InvitationRecord {
            invitation_id: invitation_id.clone(),
            verifying_key,
            nonce,
            issued_at_millis: now_millis,
            expires_at_millis,
            used: false,
            attempts: 0,
        });
        Ok(SpawnInvitation {
            invitation_id,
            body_id: self.body_id.clone(),
            nonce,
            issued_at_millis: now_millis,
            expires_at_millis,
            secret,
        })
    }

    pub fn complete_spawn(
        &mut self,
        membership: &mut BodyMembership,
        advertisement: &HostAdvertisement,
        proof: &SpawnAdmissionProof,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<MembershipCredential, AdmissionRefusal> {
        let index = self
            .invitations
            .iter()
            .position(|invitation| invitation.invitation_id == proof.invitation_id)
            .ok_or(AdmissionRefusal::UnknownInvitation)?;
        let invitation = self.invitations[index].clone();
        if invitation.used {
            return Err(AdmissionRefusal::Replay);
        }
        if invitation.attempts >= MAX_ADMISSION_ATTEMPTS {
            return Err(AdmissionRefusal::AttemptsExhausted);
        }
        if proof.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if proof.host_id != advertisement.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if proof.boot_id != advertisement.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        if proof.nonce != invitation.nonce {
            return Err(AdmissionRefusal::StaleNonce);
        }
        if now_millis > invitation.expires_at_millis {
            return Err(AdmissionRefusal::Expired);
        }
        let transcript = admission_transcript(
            "spawn-invitation-v1",
            &self.body_id,
            invitation.invitation_id.as_str(),
            advertisement.host_id.as_str(),
            advertisement.boot_id.as_str(),
            advertisement.offer_generation,
            &invitation.nonce,
            invitation.expires_at_millis,
        );
        verify_signature(invitation.verifying_key, &transcript, proof.signature).map_err(
            |refusal| {
                self.invitations[index].attempts =
                    self.invitations[index].attempts.saturating_add(1);
                if self.invitations[index].attempts >= MAX_ADMISSION_ATTEMPTS {
                    AdmissionRefusal::AttemptsExhausted
                } else {
                    refusal
                }
            },
        )?;
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        self.ensure_continuity_capacity()?;
        let admission_id = AdmissionId::bound(bind_identity(
            "spawn-admission",
            &[
                self.body_id.as_str(),
                invitation.invitation_id.as_str(),
                advertisement.host_id.as_str(),
                advertisement.boot_id.as_str(),
            ],
            now_millis,
        ));
        let mut next_membership = membership.clone();
        let credential = self.attach(
            &mut next_membership,
            advertisement,
            now_millis,
            &admission_id,
            &signs,
        )?;
        self.retain_receipt(admission_id, credential.clone())?;
        self.retain_continuity_key(
            credential.part_id.clone(),
            credential.host_id.clone(),
            invitation.verifying_key,
        );
        self.invitations[index].used = true;
        *membership = next_membership;
        Ok(credential)
    }
}
