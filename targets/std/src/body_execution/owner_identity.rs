//! One explicitly prepared owner identity with original IMAGE byte custody.
//! Authentication binds a startup scope; it grants no execution or sound.
use crate::{kernel_preparation::ImageMemoryLease, StdHost};
use conduit_body::{
    BodyId, BodyPlayIdentity, FederationChallenge, FederationPeer, MembershipCredentialId, PartId,
    SpawnInvitationSecret, WakeId,
};
use conduit_core::{LineSecurity, PlanId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_ID_BYTES: usize = 256;
const MAX_AUTH_TTL_MILLIS: u64 = 30_000;

/// Original owner key custody, created only by explicit local preparation.
/// There is no key accessor, cloning, serialization or arbitrary signing API.
pub struct StartupOwnerIdentity {
    key: SpawnInvitationSecret,
    peer: FederationPeer,
    // The key and its public identity drop before the original charge.
    _memory: Option<ImageMemoryLease>,
}

/// The original Body scope bound to both peers on one selected TLS channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupOwnerChallenge {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub plan_id: PlanId,
    pub global_play: BodyPlayIdentity,
    pub part_id: PartId,
    pub credential_id: MembershipCredentialId,
    pub member_offer_digest: [u8; 32],
    pub transport_binding_sha256: [u8; 32],
    pub federation: FederationChallenge,
}

impl StdHost {
    /// Explicit local setup, before a Host loan or Started. The owner retains
    /// this value and returns its existing peer on subsequent setup requests.
    /// Resume and challenge signing never call this constructor.
    pub fn prepare_startup_owner_identity(&self) -> Result<StartupOwnerIdentity, String> {
        self.kernel_resources.require_owner_identity_preparation()?;
        let host = &self.advertisement;
        if !bounded_id(host.host_id.as_str())
            || !bounded_id(host.boot_id.as_str())
            || host.offer_generation.0 == 0
        {
            return Err("owner identity requires the exact current Host incarnation".into());
        }
        // This fixed owner is stored inline by the product Owner. Its only
        // heap owners are the public peer IDs and the one private lease Arc.
        let bytes = core::mem::size_of::<StartupOwnerIdentity>()
            .checked_add(host.host_id.as_str().len())
            .and_then(|n| n.checked_add(host.boot_id.as_str().len()))
            .and_then(|n| n.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("owner identity backing overflow")?;
        let memory = if self.kernel_resources.has_image_bounds() {
            Some(self.kernel_resources.fund_tick_preparation(bytes)?)
        } else {
            None
        };
        let mut entropy = [0; 32];
        if getrandom::fill(&mut entropy).is_err() {
            entropy.fill(0);
            return Err("owner identity randomness unavailable".into());
        }
        let key = SpawnInvitationSecret::from_csprng_bytes(entropy)
            .map_err(|_| "owner identity randomness invalid")?;
        entropy.fill(0);
        let peer = FederationPeer {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            verifying_key: key.verifying_key(),
        };
        Ok(StartupOwnerIdentity {
            key,
            peer,
            _memory: memory,
        })
    }
}

impl StartupOwnerIdentity {
    pub fn peer(&self) -> &FederationPeer {
        &self.peer
    }

    pub fn sign_startup_challenge(
        &self,
        challenge: &StartupOwnerChallenge,
        now_millis: u64,
    ) -> Result<[u8; 64], String> {
        challenge.validate(&self.peer, now_millis)?;
        let transcript = conduit_body::challenge_transcript_digest(&challenge.federation);
        Ok(self.key.sign(&transcript))
    }
}

impl StartupOwnerChallenge {
    /// Bind the caller's exact scope to the existing federation protocol.
    /// Session and Line IDs are derived, never included recursively in the hash.
    pub fn bind_session(mut self) -> Result<Self, String> {
        self.validate_shape()?;
        self.federation.session_id = format!("startup/{}", hex(&self.scope_digest())).into();
        self.federation.line_id = format!("startup-tls/{}", hex(&self.line_digest())).into();
        Ok(self)
    }

    /// The expected owner must come from independent retained pin custody.
    /// The product separately checks current membership, offers and authority.
    pub fn validate(&self, owner: &FederationPeer, now_millis: u64) -> Result<(), String> {
        self.validate_shape()?;
        if &self.federation.responder != owner
            || self.federation.expires_at_millis <= now_millis
            || self.federation.expires_at_millis - now_millis > MAX_AUTH_TTL_MILLIS
            || !matches_hex_id(
                self.federation.session_id.as_str(),
                "startup/",
                &self.scope_digest(),
            )
            || !matches_hex_id(
                self.federation.line_id.as_str(),
                "startup-tls/",
                &self.line_digest(),
            )
        {
            return Err("startup authentication differs from retained owner or exact scope".into());
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), String> {
        let f = &self.federation;
        let ids = [
            self.body_id.as_str(),
            self.wake_id.as_str(),
            self.plan_id.as_str(),
            self.part_id.as_str(),
            self.credential_id.as_str(),
            self.global_play.body_id.as_str(),
            self.global_play.wake_id.as_str(),
            self.global_play.plan_id.as_str(),
            self.global_play.active_play_id.as_str(),
            f.initiator.host_id.as_str(),
            f.initiator.boot_id.as_str(),
            f.responder.host_id.as_str(),
            f.responder.boot_id.as_str(),
        ];
        if ids.iter().any(|id| !bounded_id(id))
            || self.global_play.body_id != self.body_id
            || self.global_play.wake_id != self.wake_id
            || self.global_play.plan_id != self.plan_id
            || f.line_security != LineSecurity::AuthenticatedEncrypted
            || f.initiator.host_id == f.responder.host_id
            || f.initiator.offer_generation.0 == 0
            || f.responder.offer_generation.0 == 0
            || f.initiator.verifying_key == [0; 32]
            || f.responder.verifying_key == [0; 32]
            || f.nonce == [0; 32]
            || f.maximum_frames == 0
            || self.member_offer_digest == [0; 32]
            || self.transport_binding_sha256 == [0; 32]
        {
            return Err("startup authentication has an invalid or unrelated scope".into());
        }
        Ok(())
    }

    fn scope_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        put(&mut hash, "conduit/body-startup-owner-scope/v1");
        for value in [
            self.body_id.as_str(),
            self.wake_id.as_str(),
            self.plan_id.as_str(),
            self.global_play.body_id.as_str(),
            self.global_play.wake_id.as_str(),
            self.global_play.plan_id.as_str(),
            self.global_play.active_play_id.as_str(),
            self.part_id.as_str(),
            self.credential_id.as_str(),
        ] {
            put(&mut hash, value);
        }
        hash.update(self.global_play.play_sequence.to_le_bytes());
        hash.update(self.member_offer_digest);
        hash.update(self.transport_binding_sha256);
        for peer in [&self.federation.initiator, &self.federation.responder] {
            put(&mut hash, peer.host_id.as_str());
            put(&mut hash, peer.boot_id.as_str());
            hash.update(peer.offer_generation.0.to_le_bytes());
            hash.update(peer.verifying_key);
        }
        hash.update(self.federation.nonce);
        hash.update(self.federation.expires_at_millis.to_le_bytes());
        hash.update(u64::from(self.federation.maximum_frames).to_le_bytes());
        hash.finalize().into()
    }

    fn line_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.transport_binding_sha256);
        hash.update(self.federation.nonce);
        hash.finalize().into()
    }
}

fn bounded_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_ID_BYTES
}
fn put(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u32).to_le_bytes());
    hash.update(value.as_bytes());
}
fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("String formatting");
    }
    output
}
fn matches_hex_id(value: &str, prefix: &str, bytes: &[u8; 32]) -> bool {
    let Some(text) = value.strip_prefix(prefix) else {
        return false;
    };
    if text.len() != 64 {
        return false;
    }
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .zip(text.as_bytes().as_chunks::<2>().0)
        .all(|(byte, pair)| {
            *pair
                == [
                    DIGITS[usize::from(byte >> 4)],
                    DIGITS[usize::from(byte & 15)],
                ]
        })
}

#[cfg(test)]
mod tests;
