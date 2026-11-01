//! A provisioned administrative signer authorizes one exact Body name change.
//! The trusted Host supplies its pinned owner key; request data cannot select it.
use alloc::{string::String, vec::Vec};
use conduit_core::{BootId, HostId};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{BodyId, MAX_BODY_FRIENDLY_NAME_BYTES, MAX_LIFECYCLE_ID_BYTES};

pub const BODY_RENAME_REQUEST_SCHEMA: &str = "conduit.body/rename-request@1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyRenameRequest {
    pub schema: String,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub expected_sequence: u64,
    pub nonce: [u8; 32],
    pub new_name: String,
    pub attribution: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyRenameRefusal {
    InvalidRequest,
    Denied,
    WrongBody,
    Stale,
    Host,
    Fulfilled,
    Capacity,
    Evidence,
}

/// Only signature verification can construct this administrative intent.
#[derive(Clone, Debug)]
pub struct AuthorizedBodyRename {
    pub(crate) request: BodyRenameRequest,
    pub(crate) owner_key: [u8; 32],
    pub(crate) digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyRenameResult {
    pub body_id: BodyId,
    pub friendly_name: String,
    pub sequence: u64,
    pub sign_id: conduit_core::SignId,
    pub request_digest: [u8; 32],
    /// Exact renamed biography record authenticated by this completion.
    pub record_digest: Option<[u8; 32]>,
    pub changed: bool,
}

impl BodyRenameRequest {
    /// Stable, domain-separated transcript shared by CLI and all Root adapters.
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/rename-request@1\0");
        for value in [
            self.schema.as_str(),
            self.body_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
            self.new_name.as_str(),
            self.attribution.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.expected_sequence.to_le_bytes());
        hash.update(self.nonce);
        hash.finalize().into()
    }

    pub fn authorize(
        &self,
        pinned_owner_key: &[u8; 32],
    ) -> Result<AuthorizedBodyRename, BodyRenameRefusal> {
        if self.schema != BODY_RENAME_REQUEST_SCHEMA
            || self.nonce == [0; 32]
            || !valid_name(&self.new_name)
            || self.attribution.trim().is_empty()
            || self.attribution.len() > MAX_LIFECYCLE_ID_BYTES
            || self.attribution.chars().any(char::is_control)
            || [
                self.body_id.as_str(),
                self.host_id.as_str(),
                self.boot_id.as_str(),
            ]
            .iter()
            .any(|id| id.is_empty() || id.len() > MAX_LIFECYCLE_ID_BYTES)
        {
            return Err(BodyRenameRefusal::InvalidRequest);
        }
        let key =
            VerifyingKey::from_bytes(pinned_owner_key).map_err(|_| BodyRenameRefusal::Denied)?;
        if key.is_weak() {
            return Err(BodyRenameRefusal::Denied);
        }
        let signature =
            Signature::from_slice(&self.signature).map_err(|_| BodyRenameRefusal::Denied)?;
        let digest = self.transcript();
        key.verify_strict(&digest, &signature)
            .map_err(|_| BodyRenameRefusal::Denied)?;
        Ok(AuthorizedBodyRename {
            request: self.clone(),
            owner_key: *pinned_owner_key,
            digest,
        })
    }
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_BODY_FRIENDLY_NAME_BYTES
        && !name.chars().any(char::is_control)
}

/// A Root-signed completion is issued only after the adapter's durable commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyRenameReceipt {
    pub schema: String,
    pub result: BodyRenameResult,
    pub root_key: [u8; 32],
    pub signature: Vec<u8>,
}
impl BodyRenameReceipt {
    pub fn transcript(&self) -> Result<[u8; 32], BodyRenameRefusal> {
        if !valid_name(&self.result.friendly_name)
            || [self.result.body_id.as_str(), self.result.sign_id.as_str()]
                .iter()
                .any(|id| id.is_empty() || id.len() > MAX_LIFECYCLE_ID_BYTES)
        {
            return Err(BodyRenameRefusal::Evidence);
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/rename-receipt@1\0");
        hash.update(self.root_key);
        for value in [
            self.result.body_id.as_str(),
            self.result.friendly_name.as_str(),
            self.result.sign_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.result.sequence.to_le_bytes());
        hash.update(self.result.request_digest);
        hash.update([u8::from(self.result.record_digest.is_some())]);
        if let Some(digest) = self.result.record_digest {
            hash.update(digest);
        }
        hash.update([u8::from(self.result.changed)]);
        Ok(hash.finalize().into())
    }
    /// Verify the previous/new names and attribution against the Root-signed
    /// completion, in addition to verifying the request and current name.
    pub fn verify_record(
        &self,
        pinned_root: &[u8; 32],
        request: &BodyRenameRequest,
        record: &crate::BodyBiographyRecord,
    ) -> Result<(), BodyRenameRefusal> {
        self.verify(pinned_root, request)?;
        let crate::BodyBiographyRecordKind::Renamed {
            new_name,
            attribution,
            request_digest,
            ..
        } = &record.kind
        else {
            return Err(BodyRenameRefusal::Evidence);
        };
        if new_name != &self.result.friendly_name
            || attribution != &request.attribution
            || request_digest != &self.result.request_digest
        {
            return Err(BodyRenameRefusal::Evidence);
        }
        if self.result.record_digest
            != Some(
                record
                    .rename_digest()
                    .map_err(|_| BodyRenameRefusal::Evidence)?,
            )
            || record.sequence != self.result.sequence
            || record.sign_id != self.result.sign_id
        {
            return Err(BodyRenameRefusal::Evidence);
        }
        Ok(())
    }
    pub fn verify(
        &self,
        pinned_root: &[u8; 32],
        request: &BodyRenameRequest,
    ) -> Result<(), BodyRenameRefusal> {
        if self.schema != "conduit.body/rename-receipt@1"
            || &self.root_key != pinned_root
            || self.result.body_id != request.body_id
            || self.result.friendly_name != request.new_name
            || self.result.request_digest != request.transcript()
            || self.result.sign_id.as_str().is_empty()
            || (self.result.changed && self.result.record_digest.is_none())
            || self.result.sequence < request.expected_sequence
        {
            return Err(BodyRenameRefusal::Evidence);
        }
        let key = VerifyingKey::from_bytes(pinned_root).map_err(|_| BodyRenameRefusal::Denied)?;
        if key.is_weak() {
            return Err(BodyRenameRefusal::Denied);
        }
        let signature =
            Signature::from_slice(&self.signature).map_err(|_| BodyRenameRefusal::Denied)?;
        key.verify_strict(&self.transcript()?, &signature)
            .map_err(|_| BodyRenameRefusal::Denied)
    }
}
impl BodyRenameRequest {
    /// The caller owns provisioning custody and must erase its temporary seed.
    pub fn sign(&mut self, owner_seed: &[u8; 32]) {
        use ed25519_dalek::{Signer, SigningKey};
        self.signature = SigningKey::from_bytes(owner_seed)
            .sign(&self.transcript())
            .to_bytes()
            .to_vec();
    }
}
