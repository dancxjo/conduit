//! Read-only current-membership receipt recovery, distinct from admission.
//! No invitation replay, presence refresh, Plan or execution grant is created.
use alloc::{string::String, vec::Vec};
use conduit_body::{BodyId, MembershipCredential};
use conduit_core::{BootId, HostId};
use sha2::{Digest, Sha256};
pub const REQUEST: &str = "conduit.environment/observer-recovery-request@1";
pub const RECEIPT: &str = "conduit.environment/current-observer@1";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub nonce: [u8; 32],
    pub signature: Vec<u8>,
}
fn text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}
impl Request {
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/observer-recovery-request@1\0");
        for value in [
            self.body_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
        ] {
            text(&mut hash, value);
        }
        hash.update(self.nonce);
        hash.finalize().into()
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub credential: MembershipCredential,
    pub membership_revision: u64,
    pub producer_host: HostId,
    pub producer_boot: BootId,
    pub producer_key: [u8; 32],
    pub request_nonce: [u8; 32],
    pub signature: Vec<u8>,
}
impl Receipt {
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/current-observer@1\0");
        for value in [
            self.credential.credential_id.as_str(),
            self.credential.body_id.as_str(),
            self.credential.part_id.as_str(),
            self.credential.host_id.as_str(),
            self.credential.boot_id.as_str(),
            self.producer_host.as_str(),
            self.producer_boot.as_str(),
        ] {
            text(&mut hash, value);
        }
        hash.update(self.credential.issued_at_millis.to_le_bytes());
        hash.update(self.membership_revision.to_le_bytes());
        hash.update(self.producer_key);
        hash.update(self.request_nonce);
        hash.finalize().into()
    }
    pub fn verify(&self, root: &[u8; 32], request: &Request) -> Result<(), &'static str> {
        if self.schema != RECEIPT
            || request.schema != REQUEST
            || request.nonce == [0; 32]
            || self.producer_key != *root
            || self.membership_revision == 0
            || self.request_nonce != request.nonce
            || self.credential.body_id != request.body_id
            || self.credential.host_id != request.host_id
            || self.credential.boot_id != request.boot_id
        {
            return Err("observer-recovery-scope");
        }
        let signature: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| "observer-recovery-signature-extent")?;
        ed25519_dalek::VerifyingKey::from_bytes(root)
            .map_err(|_| "observer-recovery-root")?
            .verify_strict(
                &self.transcript(),
                &ed25519_dalek::Signature::from_bytes(&signature),
            )
            .map_err(|_| "observer-recovery-signature")
    }
}
