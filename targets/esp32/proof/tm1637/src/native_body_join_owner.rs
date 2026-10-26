//! Provisioned-owner authorization for private USB invitation custody only.
//! Public HTTP discovery cannot supply this result or receive a bearer secret.
use alloc::{string::String, vec::Vec};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) const PROFILE: &str = "conduit.environment/private-invitation-owner@1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema: String,
    pub host: String,
    pub boot: String,
    pub body: String,
    pub root_key: [u8; 32],
    pub nonce: [u8; 32],
    pub sequence: u64,
    pub ttl_ms: u64,
    pub signature: Vec<u8>,
}
impl Request {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/private-invitation-owner@1\0");
        for id in [&self.host, &self.boot, &self.body] {
            hash.update((id.len() as u64).to_le_bytes());
            hash.update(id.as_bytes());
        }
        hash.update(self.root_key);
        hash.update(self.nonce);
        hash.update(self.sequence.to_le_bytes());
        hash.update(self.ttl_ms.to_le_bytes());
        hash.finalize().into()
    }
}
pub(crate) struct Gate {
    key: VerifyingKey,
    nonce: Option<[u8; 32]>,
    deadline_ms: u64,
    last_sequence: u64,
}
pub(crate) struct AuthorizedInvitation {
    ttl_ms: u64,
}
impl AuthorizedInvitation {
    pub(crate) fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }
}
impl Gate {
    pub(crate) fn new(owner_key: Option<[u8; 32]>) -> Result<Self, &'static str> {
        let key = VerifyingKey::from_bytes(&owner_key.ok_or("body-join-owner-disabled")?)
            .map_err(|_| "body-join-owner-key")?;
        if key.is_weak() {
            return Err("body-join-owner-key");
        }
        Ok(Self {
            key,
            nonce: None,
            deadline_ms: 0,
            last_sequence: 0,
        })
    }
    pub(crate) fn next_sequence(&self) -> Option<u64> {
        self.last_sequence.checked_add(1)
    }
    pub(crate) fn challenge(&mut self, nonce: [u8; 32], now_ms: u64) -> Result<(), &'static str> {
        if nonce == [0; 32] {
            return Err("body-join-owner-entropy");
        }
        self.deadline_ms = now_ms.checked_add(30_000).ok_or("body-join-owner-clock")?;
        self.nonce = Some(nonce);
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn authorize(
        &mut self,
        request: &Request,
        host: &str,
        boot: &str,
        body: &str,
        root_key: &[u8; 32],
        now_ms: u64,
    ) -> Result<AuthorizedInvitation, &'static str> {
        if request.schema != PROFILE
            || request.host != host
            || request.boot != boot
            || request.body != body
            || &request.root_key != root_key
            || Some(request.nonce) != self.nonce
            || now_ms > self.deadline_ms
            || request.sequence <= self.last_sequence
            || request.ttl_ms == 0
            || request.ttl_ms > conduit_body::MAX_SPAWN_INVITATION_TTL_MILLIS
        {
            return Err("body-join-owner-scope");
        }
        let signature: [u8; 64] = request
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| "body-join-owner-signature")?;
        self.key
            .verify_strict(&request.transcript(), &Signature::from_bytes(&signature))
            .map_err(|_| "body-join-owner-signature")?;
        self.nonce = None;
        self.last_sequence = request.sequence;
        Ok(AuthorizedInvitation {
            ttl_ms: request.ttl_ms,
        })
    }
}
