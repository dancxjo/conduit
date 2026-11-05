//! Exact command attribution, not permission to prepare or execute its contents.
//! Caller-owned trusted membership/peer facts and fresh issuance are mandatory.
use crate::{FederationPeer, MembershipCredential, MAX_LIFECYCLE_ID_BYTES};
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"conduit.body/host-command-attribution@1";
#[path = "host_command_retention.rs"]
mod retention;
pub use retention::{HostCommandInput, HostCommandRetentionRefusal, RetainedHostCommandStamp};

/// Public challenge facts. Deserialization/duplication does not issue a slot.
/// The sender must independently check the receiver and installed payload
/// contract before deciding whether to sign; this document grants no authority.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostCommandChallenge<'a> {
    pub body_id: &'a str,
    pub part_id: &'a str,
    pub credential_id: &'a str,
    pub receiver_host: &'a str,
    pub receiver_boot: &'a str,
    pub receiver_generation: u64,
    pub receiver_profile: &'a str,
    pub issuer_host: &'a str,
    pub issuer_boot: &'a str,
    pub issuer_generation: u64,
    pub issuer_key: [u8; 32],
    pub payload_contract: &'a str,
    pub maximum_payload_bytes: u32,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

/// Receiver-local profile facts, never taken from a command's payload.
pub struct HostCommandReceiveBounds<'a> {
    pub payload_contract: &'a str,
    pub maximum_payload_bytes: u32,
    pub lifetime_millis: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCommandAttributionRefusal {
    Context,
    Bounds,
    Clock,
    Signature,
}

/// Non-clonable, non-deserializable issuance. Every authentication attempt
/// consumes it, including failure. A fresh slot needs a fresh caller-owned nonce.
pub struct IssuedHostCommandChallenge<'a> {
    challenge: HostCommandChallenge<'a>,
}

/// Only attribution to the independently pinned issuer. It grants no membership,
/// Plan approval, reservation, Host Call possession, Line or execution right.
pub struct AttributedHostCommand<'context, 'payload> {
    challenge: HostCommandChallenge<'context>,
    payload: &'payload [u8],
}
impl<'context, 'payload> AttributedHostCommand<'context, 'payload> {
    pub fn challenge(&self) -> &HostCommandChallenge<'context> {
        &self.challenge
    }
    pub fn payload(&self) -> &'payload [u8] {
        self.payload
    }
}

impl<'a> IssuedHostCommandChallenge<'a> {
    /// `member`, `actual` and `issuer` must be independently retained receiver
    /// facts on the actual Boot. Fresh preparation uses current offers; lifecycle
    /// control must use its exact retained reservation scope. This API cannot
    /// authorize that choice. Public credential fields are not membership proof.
    /// Nonce uniqueness/unpredictability and monotonic local time are caller
    /// obligations. Reconstructing a consumed challenge is not a retry policy.
    pub fn issue(
        member: &'a MembershipCredential,
        actual: &'a HostAdvertisement,
        issuer: &'a FederationPeer,
        bounds: HostCommandReceiveBounds<'a>,
        nonce: [u8; 32],
        now_millis: u64,
    ) -> Result<Self, HostCommandAttributionRefusal> {
        use HostCommandAttributionRefusal as E;
        if member.host_id != actual.host_id
            || member.boot_id != actual.boot_id
            || actual.protocol_version != PROTOCOL_VERSION
            || actual.offer_generation.0 == 0
            || issuer.offer_generation.0 == 0
            || nonce == [0; 32]
            || issuer.verifying_key == [0; 32]
            || ![
                member.body_id.as_str(),
                member.part_id.as_str(),
                member.credential_id.as_str(),
                actual.host_id.as_str(),
                actual.boot_id.as_str(),
                actual.profile.as_str(),
                issuer.host_id.as_str(),
                issuer.boot_id.as_str(),
                bounds.payload_contract,
            ]
            .iter()
            .all(|id| !id.is_empty() && id.len() <= MAX_LIFECYCLE_ID_BYTES)
        {
            return Err(E::Context);
        }
        VerifyingKey::from_bytes(&issuer.verifying_key).map_err(|_| E::Context)?;
        if bounds.maximum_payload_bytes == 0 || bounds.lifetime_millis == 0 {
            return Err(E::Bounds);
        }
        let expires_at_millis = now_millis
            .checked_add(u64::from(bounds.lifetime_millis))
            .ok_or(E::Clock)?;
        Ok(Self {
            challenge: HostCommandChallenge {
                body_id: member.body_id.as_str(),
                part_id: member.part_id.as_str(),
                credential_id: member.credential_id.as_str(),
                receiver_host: actual.host_id.as_str(),
                receiver_boot: actual.boot_id.as_str(),
                receiver_generation: actual.offer_generation.0,
                receiver_profile: actual.profile.as_str(),
                issuer_host: issuer.host_id.as_str(),
                issuer_boot: issuer.boot_id.as_str(),
                issuer_generation: issuer.offer_generation.0,
                issuer_key: issuer.verifying_key,
                payload_contract: bounds.payload_contract,
                maximum_payload_bytes: bounds.maximum_payload_bytes,
                nonce,
                issued_at_millis: now_millis,
                expires_at_millis,
            },
        })
    }

    pub fn challenge(&self) -> &HostCommandChallenge<'a> {
        &self.challenge
    }

    /// Consumes this issuance before any caller decoding or effects. The caller
    /// must still apply its current policy, full Plan and finite admission checks.
    ///
    /// ```compile_fail
    /// use conduit_body::IssuedHostCommandChallenge;
    /// fn replay(slot: IssuedHostCommandChallenge<'_>, payload: &[u8], signature: &[u8]) {
    ///     let _ = slot.authenticate(payload, signature, 1);
    ///     let _ = slot.authenticate(payload, signature, 1);
    /// }
    /// ```
    pub fn authenticate<'p>(
        self,
        payload: &'p [u8],
        signature: &[u8],
        now_millis: u64,
    ) -> Result<AttributedHostCommand<'a, 'p>, HostCommandAttributionRefusal> {
        use HostCommandAttributionRefusal as E;
        if now_millis < self.challenge.issued_at_millis
            || now_millis >= self.challenge.expires_at_millis
        {
            return Err(E::Clock);
        }
        let transcript = host_command_attribution_transcript(&self.challenge, payload)?;
        let signature = Signature::from_slice(signature).map_err(|_| E::Signature)?;
        VerifyingKey::from_bytes(&self.challenge.issuer_key)
            .map_err(|_| E::Context)?
            .verify_strict(&transcript, &signature)
            .map_err(|_| E::Signature)?;
        Ok(AttributedHostCommand {
            challenge: self.challenge,
            payload,
        })
    }
}

/// Allocation-free signing transcript over exact bytes and the issued context.
/// Producing/signing this digest does not issue a receiving slot or approve a Plan.
pub fn host_command_attribution_transcript(
    challenge: &HostCommandChallenge<'_>,
    payload: &[u8],
) -> Result<[u8; 32], HostCommandAttributionRefusal> {
    let bytes = u32::try_from(payload.len()).map_err(|_| HostCommandAttributionRefusal::Bounds)?;
    if bytes == 0 || bytes > challenge.maximum_payload_bytes {
        return Err(HostCommandAttributionRefusal::Bounds);
    }
    let mut digest = Sha256::new();
    for bytes in [
        DOMAIN,
        challenge.body_id.as_bytes(),
        challenge.part_id.as_bytes(),
        challenge.credential_id.as_bytes(),
        challenge.receiver_host.as_bytes(),
        challenge.receiver_boot.as_bytes(),
        challenge.receiver_profile.as_bytes(),
        challenge.issuer_host.as_bytes(),
        challenge.issuer_boot.as_bytes(),
        challenge.payload_contract.as_bytes(),
    ] {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    digest.update(PROTOCOL_VERSION.to_le_bytes());
    digest.update(challenge.receiver_generation.to_le_bytes());
    digest.update(challenge.issuer_generation.to_le_bytes());
    digest.update(challenge.issuer_key);
    digest.update(challenge.maximum_payload_bytes.to_le_bytes());
    digest.update(challenge.nonce);
    digest.update(challenge.issued_at_millis.to_le_bytes());
    digest.update(challenge.expires_at_millis.to_le_bytes());
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(Sha256::digest(payload));
    Ok(digest.finalize().into())
}
