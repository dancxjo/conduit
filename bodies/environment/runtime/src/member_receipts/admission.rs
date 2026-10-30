//! Canonical signature transcript for an actual durable invitation admission.
//! This wire helper creates neither membership nor a trusted producer key.
use conduit_body::{MembershipCredential, SpawnInvitationId};
use conduit_core::{BootId, HostId, OfferGeneration};
use sha2::{Digest, Sha256};
pub const PROFILE: &str = "conduit.environment/joined-host@1";
/// Signed evidence emitted only after actual admission and durable checkpoint.
/// Deserializing this object alone supplies no independently trusted Root key.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinedHostReceipt {
    pub schema: alloc::string::String,
    pub credential: MembershipCredential,
    pub membership_revision: u64,
    pub producer_host: HostId,
    pub producer_boot: BootId,
    pub producer_key: [u8; 32],
    pub invitation_id: SpawnInvitationId,
    pub offer_generation: OfferGeneration,
    pub signature: alloc::vec::Vec<u8>,
}
impl JoinedHostReceipt {
    pub fn scope(&self) -> JoinedHostScope<'_> {
        JoinedHostScope {
            credential: &self.credential,
            membership_revision: self.membership_revision,
            producer_host: &self.producer_host,
            producer_boot: &self.producer_boot,
            producer_key: &self.producer_key,
            invitation_id: &self.invitation_id,
            offer_generation: self.offer_generation,
        }
    }
    pub fn verify(&self, expected_root: &[u8; 32]) -> Result<(), &'static str> {
        if self.schema != PROFILE
            || &self.producer_key != expected_root
            || self.membership_revision == 0
            || self.offer_generation.0 == 0
        {
            return Err("joined-host-scope");
        }
        let key = ed25519_dalek::VerifyingKey::from_bytes(expected_root)
            .map_err(|_| "joined-host-root-key")?;
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| "joined-host-signature-extent")?;
        key.verify_strict(
            &transcript(&self.scope()),
            &ed25519_dalek::Signature::from_bytes(&signature),
        )
        .map_err(|_| "joined-host-signature")
    }
}
pub struct JoinedHostScope<'a> {
    pub credential: &'a MembershipCredential,
    pub membership_revision: u64,
    pub producer_host: &'a HostId,
    pub producer_boot: &'a BootId,
    pub producer_key: &'a [u8; 32],
    pub invitation_id: &'a SpawnInvitationId,
    pub offer_generation: OfferGeneration,
}
fn text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}
pub fn transcript(scope: &JoinedHostScope<'_>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/joined-host@1\0");
    for value in [
        scope.credential.credential_id.as_str(),
        scope.credential.body_id.as_str(),
        scope.credential.part_id.as_str(),
        scope.credential.host_id.as_str(),
        scope.credential.boot_id.as_str(),
        scope.producer_host.as_str(),
        scope.producer_boot.as_str(),
        scope.invitation_id.as_str(),
    ] {
        text(&mut hash, value);
    }
    hash.update(scope.credential.issued_at_millis.to_le_bytes());
    hash.update(scope.membership_revision.to_le_bytes());
    hash.update(scope.offer_generation.0.to_le_bytes());
    hash.update(scope.producer_key);
    hash.finalize().into()
}
