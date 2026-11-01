//! Affine challenge and continuity-authenticated equipment observation.
//! Transport and workspace admission remain the owning Root's obligations.
use super::{validate_shape, CurrentHostOfferError, CurrentHostOffers};
use crate::{
    AdmissionManager, BodyBiographyEvidence, BodyId, BodyMembershipRevision, MembershipState,
    PartId,
};
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_CURRENT_HOST_OFFER_CHALLENGE_MILLIS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentHostOfferChallenge {
    pub body_id: BodyId,
    pub root_host: HostId,
    pub root_boot: BootId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub membership_revision: BodyMembershipRevision,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentHostOfferAuthenticationError {
    Scope,
    Expired,
    Advertisement,
    Signature,
    Serialization,
    Inventory(CurrentHostOfferError),
}
use CurrentHostOfferAuthenticationError as Error;

/// Not deserializable or clonable: only local issuance creates pending custody.
pub struct PendingCurrentHostOfferChallenge(CurrentHostOfferChallenge);
/// Owns the original complete descriptor; no cloning or decoding creates this.
pub struct VerifiedCurrentHostOffer {
    challenge: CurrentHostOfferChallenge,
    advertisement: HostAdvertisement,
}

impl CurrentHostOfferChallenge {
    /// Allocation-free canonical complete-descriptor signature transcript.
    /// This computes bytes to sign, not evidence of authentication or admission.
    pub fn signing_transcript(&self, advertisement: &HostAdvertisement) -> Result<[u8; 32], Error> {
        let descriptor = super::descriptor_hash::advertisement_digest(advertisement)
            .map_err(|_| Error::Serialization)?;
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/current-host-offer-proof@1\0");
        postcard::serialize_with_flavor(
            &(self, descriptor),
            crate::postcard_hash::PostcardSha256(hash),
        )
        .map_err(|_| Error::Serialization)
    }
    fn check(
        &self,
        evidence: &BodyBiographyEvidence,
        root_host: &HostId,
        root_boot: &BootId,
        now: u64,
    ) -> Result<(), Error> {
        if now < self.issued_at_millis || now > self.expires_at_millis {
            return Err(Error::Expired);
        }
        if self.body_id != evidence.body_id
            || self.body_id != evidence.membership.body_id
            || &self.root_host != root_host
            || &self.root_boot != root_boot
            || self.membership_revision != evidence.membership.revision
            || self.nonce == [0; 32]
            || !evidence.membership.parts.iter().any(|part| {
                part.part_id == self.part_id
                    && part.state == MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        current.host_id == self.host_id
                            && current.boot_id == self.boot_id
                            && current.offer_generation == self.offer_generation
                    })
            })
        {
            return Err(Error::Scope);
        }
        Ok(())
    }
}
impl PendingCurrentHostOfferChallenge {
    /// The caller is the actual Root owner and supplies its current Boot and fresh
    /// entropy. Keep this owner local and discard all pending owners on restart.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        evidence: &BodyBiographyEvidence,
        root_host: HostId,
        root_boot: BootId,
        part_id: PartId,
        nonce: [u8; 32],
        now: u64,
        expires: u64,
    ) -> Result<Self, Error> {
        if expires <= now
            || expires - now > MAX_CURRENT_HOST_OFFER_CHALLENGE_MILLIS
            || root_host.as_str().is_empty()
            || root_boot.as_str().is_empty()
        {
            return Err(Error::Scope);
        }
        let current = evidence
            .membership
            .parts
            .iter()
            .find(|part| part.part_id == part_id && part.state == MembershipState::Admitted)
            .and_then(|part| part.current.as_ref())
            .ok_or(Error::Scope)?;
        let challenge = CurrentHostOfferChallenge {
            body_id: evidence.body_id.clone(),
            root_host,
            root_boot,
            part_id,
            host_id: current.host_id.clone(),
            boot_id: current.boot_id.clone(),
            offer_generation: current.offer_generation,
            membership_revision: evidence.membership.revision,
            nonce,
            issued_at_millis: now,
            expires_at_millis: expires,
        };
        challenge.check(evidence, &challenge.root_host, &challenge.root_boot, now)?;
        Ok(Self(challenge))
    }
    pub fn challenge(&self) -> &CurrentHostOfferChallenge {
        &self.0
    }
    /// Consumes pending custody on every attempt, including invalid proofs.
    #[allow(clippy::too_many_arguments)]
    pub fn verify(
        self,
        manager: &AdmissionManager,
        evidence: &BodyBiographyEvidence,
        root_host: &HostId,
        root_boot: &BootId,
        advertisement: HostAdvertisement,
        signature: &[u8; 64],
        now: u64,
    ) -> Result<VerifiedCurrentHostOffer, Error> {
        self.0.check(evidence, root_host, root_boot, now)?;
        validate_shape(&advertisement).map_err(|_| Error::Advertisement)?;
        if advertisement.host_id != self.0.host_id
            || advertisement.boot_id != self.0.boot_id
            || advertisement.offer_generation != self.0.offer_generation
        {
            return Err(Error::Scope);
        }
        let transcript = self.0.signing_transcript(&advertisement)?;
        manager
            .verify_admitted_message(
                &evidence.membership,
                &self.0.part_id,
                &self.0.host_id,
                &self.0.boot_id,
                &transcript,
                signature,
            )
            .map_err(|_| Error::Signature)?;
        Ok(VerifiedCurrentHostOffer {
            challenge: self.0,
            advertisement,
        })
    }
}
impl CurrentHostOffers {
    /// Recheck exact current membership after verification; creates no Plan,
    /// reservation, effect grant or durable assertion of current equipment.
    pub fn observe_verified(
        &mut self,
        evidence: &BodyBiographyEvidence,
        root_host: &HostId,
        root_boot: &BootId,
        verified: VerifiedCurrentHostOffer,
        now: u64,
    ) -> Result<(), Error> {
        verified
            .challenge
            .check(evidence, root_host, root_boot, now)?;
        self.observe(evidence, verified.advertisement)
            .map_err(Error::Inventory)
    }
}
