//! Mutually authenticated, non-transitive receiving profile for remote effects.
//!
//! Authentication attributes a frame. Membership and the receiver-local
//! #3072 capability table independently decide whether it may reach an effect.

use crate::{AuthenticatedHostObservation, PartMembership};
use alloc::vec::Vec;
use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim, BaseOperationLease, BootId,
    HostId, LineId, LineSecurity, LinkBindingId, OfferGeneration,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAXIMUM_FEDERATION_NONCE_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationPeer {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub verifying_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationChallenge {
    pub session_id: LinkBindingId,
    pub line_id: LineId,
    pub line_security: LineSecurity,
    pub initiator: FederationPeer,
    pub responder: FederationPeer,
    pub nonce: [u8; MAXIMUM_FEDERATION_NONCE_BYTES],
    pub expires_at_millis: u64,
    pub maximum_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOperationFrame {
    pub session_id: LinkBindingId,
    pub line_id: LineId,
    pub line_security: LineSecurity,
    pub sender_host_id: HostId,
    pub sender_boot_id: BootId,
    pub receiver_host_id: HostId,
    pub receiver_boot_id: BootId,
    pub sequence: u32,
    pub claim: BaseOperationClaim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationInspection {
    pub session_id: LinkBindingId,
    pub line_id: LineId,
    pub line_security: LineSecurity,
    pub authenticated_peer_host: HostId,
    pub authenticated_peer_boot: BootId,
    pub peer_offer_generation: OfferGeneration,
    pub membership_current: bool,
    pub next_sequence: u32,
    pub maximum_frames: u32,
    pub credential_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederationRefusal {
    EmptyIdentity,
    Expired,
    InvalidBounds,
    InvalidPeerKey,
    MutualAuthenticationFailed,
    WrongSession,
    Redirected,
    StaleBoot,
    Replay,
    FrameCapacity,
    PeerNotCurrentMember,
    CapabilityRefused,
    CredentialRevoked,
}

pub struct FederatedReceivingSession {
    challenge: FederationChallenge,
    peer_key: VerifyingKey,
    next_sequence: u32,
    credential_revoked: bool,
}

impl FederatedReceivingSession {
    pub fn establish(
        challenge: FederationChallenge,
        initiator_signature: [u8; 64],
        responder_signature: [u8; 64],
        now_millis: u64,
    ) -> Result<Self, FederationRefusal> {
        validate_challenge(&challenge, now_millis)?;
        let transcript = challenge_transcript(&challenge);
        let initiator_key = VerifyingKey::from_bytes(&challenge.initiator.verifying_key)
            .map_err(|_| FederationRefusal::InvalidPeerKey)?;
        let responder_key = VerifyingKey::from_bytes(&challenge.responder.verifying_key)
            .map_err(|_| FederationRefusal::InvalidPeerKey)?;
        initiator_key
            .verify(&transcript, &Signature::from_bytes(&initiator_signature))
            .and_then(|()| {
                responder_key.verify(&transcript, &Signature::from_bytes(&responder_signature))
            })
            .map_err(|_| FederationRefusal::MutualAuthenticationFailed)?;
        Ok(Self {
            challenge,
            peer_key: initiator_key,
            next_sequence: 0,
            credential_revoked: false,
        })
    }

    pub fn inspection(&self, membership: &[PartMembership]) -> FederationInspection {
        FederationInspection {
            session_id: self.challenge.session_id.clone(),
            line_id: self.challenge.line_id.clone(),
            line_security: self.challenge.line_security,
            authenticated_peer_host: self.challenge.initiator.host_id.clone(),
            authenticated_peer_boot: self.challenge.initiator.boot_id.clone(),
            peer_offer_generation: self.challenge.initiator.offer_generation,
            membership_current: current_member(membership, &self.challenge.initiator),
            next_sequence: self.next_sequence,
            maximum_frames: self.challenge.maximum_frames,
            credential_current: !self.credential_revoked,
        }
    }

    pub fn revoke_credential(&mut self) {
        self.credential_revoked = true;
    }

    pub fn authorize(
        &mut self,
        frame: &FederatedOperationFrame,
        signature: [u8; 64],
        now_millis: u64,
        membership: &[PartMembership],
        capabilities: &mut BaseCapabilityTable,
        handle: &BaseCapabilityHandle,
    ) -> Result<BaseOperationLease, FederationRefusal> {
        if now_millis > self.challenge.expires_at_millis {
            return Err(FederationRefusal::Expired);
        }
        if self.credential_revoked {
            return Err(FederationRefusal::CredentialRevoked);
        }
        if frame.session_id != self.challenge.session_id
            || frame.line_id != self.challenge.line_id
            || frame.line_security != self.challenge.line_security
        {
            return Err(FederationRefusal::WrongSession);
        }
        if frame.sender_host_id != self.challenge.initiator.host_id
            || frame.receiver_host_id != self.challenge.responder.host_id
        {
            return Err(FederationRefusal::Redirected);
        }
        if frame.sender_boot_id != self.challenge.initiator.boot_id
            || frame.receiver_boot_id != self.challenge.responder.boot_id
        {
            return Err(FederationRefusal::StaleBoot);
        }
        if frame.sequence != self.next_sequence {
            return Err(FederationRefusal::Replay);
        }
        if frame.sequence >= self.challenge.maximum_frames {
            return Err(FederationRefusal::FrameCapacity);
        }
        self.peer_key
            .verify(
                &operation_transcript(frame),
                &Signature::from_bytes(&signature),
            )
            .map_err(|_| FederationRefusal::MutualAuthenticationFailed)?;
        if !current_member(membership, &self.challenge.initiator) {
            return Err(FederationRefusal::PeerNotCurrentMember);
        }
        let lease = capabilities
            .authorize(handle, &frame.claim)
            .map_err(|_| FederationRefusal::CapabilityRefused)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(FederationRefusal::FrameCapacity)?;
        Ok(lease)
    }
}

pub fn challenge_transcript(challenge: &FederationChallenge) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"conduit/federation-session/v1");
    hash_identity(&mut hash, challenge.session_id.as_str());
    hash_identity(&mut hash, challenge.line_id.as_str());
    hash.update([match challenge.line_security {
        LineSecurity::ProcessBoundary => 0,
        LineSecurity::PhysicalPossession => 1,
        LineSecurity::PlaintextNetwork => 2,
        LineSecurity::AuthenticatedEncrypted => 3,
    }]);
    hash_peer(&mut hash, &challenge.initiator);
    hash_peer(&mut hash, &challenge.responder);
    hash.update(challenge.nonce);
    hash.update(challenge.expires_at_millis.to_le_bytes());
    hash.update(challenge.maximum_frames.to_le_bytes());
    hash.finalize().to_vec()
}

pub fn operation_transcript(frame: &FederatedOperationFrame) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"conduit/federation-operation/v1");
    for value in [
        frame.session_id.as_str(),
        frame.line_id.as_str(),
        frame.sender_host_id.as_str(),
        frame.sender_boot_id.as_str(),
        frame.receiver_host_id.as_str(),
        frame.receiver_boot_id.as_str(),
        frame.claim.host_id.as_str(),
        frame.claim.boot_id.as_str(),
        frame.claim.base_instance_id.as_str(),
        frame.claim.plan_id.as_str(),
        frame.claim.active_play_id.as_str(),
        frame.claim.implementation_id.as_str(),
        frame.claim.operation_contract_id.as_str(),
        frame.claim.subject_kind.as_str(),
        frame.claim.resource_pool_id.as_str(),
        frame.claim.resource_generation_id.0.as_str(),
        frame.claim.envelope_id.as_str(),
    ] {
        hash_identity(&mut hash, value);
    }
    hash.update(frame.sequence.to_le_bytes());
    hash.update(frame.claim.base_provider_generation.to_le_bytes());
    hash.update(frame.claim.parameter_bytes.to_le_bytes());
    hash.update(frame.claim.work_units.to_le_bytes());
    hash.finalize().to_vec()
}

fn validate_challenge(
    challenge: &FederationChallenge,
    now_millis: u64,
) -> Result<(), FederationRefusal> {
    if challenge.session_id.as_str().is_empty()
        || challenge.line_id.as_str().is_empty()
        || challenge.initiator.host_id.as_str().is_empty()
        || challenge.initiator.boot_id.as_str().is_empty()
        || challenge.responder.host_id.as_str().is_empty()
        || challenge.responder.boot_id.as_str().is_empty()
    {
        return Err(FederationRefusal::EmptyIdentity);
    }
    if challenge.maximum_frames == 0 {
        return Err(FederationRefusal::InvalidBounds);
    }
    if now_millis > challenge.expires_at_millis {
        return Err(FederationRefusal::Expired);
    }
    Ok(())
}

fn current_member(membership: &[PartMembership], peer: &FederationPeer) -> bool {
    membership.iter().any(|part| {
        part.is_present()
            && part.current.as_ref().is_some_and(
                |AuthenticatedHostObservation {
                     host_id,
                     boot_id,
                     offer_generation,
                     ..
                 }| {
                    host_id == &peer.host_id
                        && boot_id == &peer.boot_id
                        && offer_generation == &peer.offer_generation
                },
            )
    })
}

fn hash_peer(hash: &mut Sha256, peer: &FederationPeer) {
    hash_identity(hash, peer.host_id.as_str());
    hash_identity(hash, peer.boot_id.as_str());
    hash.update(peer.offer_generation.0.to_le_bytes());
    hash.update(peer.verifying_key);
}

fn hash_identity(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

#[cfg(test)]
mod tests;
