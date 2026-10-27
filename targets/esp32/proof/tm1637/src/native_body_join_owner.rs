//! Provisioned-owner authorization for private USB invitation and biography custody.
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

/// Physical owner custody is deliberately distinct from invitation issuance and
/// admitted Body membership. Its signature names one actual archive offer.
pub(crate) const BIOGRAPHY_PROFILE: &str = "conduit.environment/biography-owner-custody@1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BiographyChallenge {
    pub host: String,
    pub boot: String,
    pub body: String,
    pub root_key: [u8; 32],
    pub nonce: [u8; 32],
    pub sequence: u64,
    pub offer: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BiographyRequest {
    pub schema: String,
    pub challenge: BiographyChallenge,
    pub signature: Vec<u8>,
}
impl BiographyRequest {
    pub(crate) fn transcript(&self) -> Result<[u8; 32], &'static str> {
        let c = &self.challenge;
        if self.schema != BIOGRAPHY_PROFILE
            || c.sequence == 0
            || c.nonce == [0; 32]
            || c.offer == [0; 32]
            || [&c.host, &c.boot, &c.body].iter().any(|id| {
                id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_graphic())
            })
        {
            return Err("biography-owner-scope");
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/biography-owner-custody@1\0");
        for id in [&c.host, &c.boot, &c.body] {
            hash.update((id.len() as u64).to_le_bytes());
            hash.update(id.as_bytes());
        }
        hash.update(c.root_key);
        hash.update(c.nonce);
        hash.update(c.sequence.to_le_bytes());
        hash.update(c.offer);
        Ok(hash.finalize().into())
    }
}
/// Sender proof only. A Store must still compare its actual current snapshot
/// and consume this object only for the named biography offer.
pub(crate) struct AuthorizedOwnerBiography {
    body: String,
    root: [u8; 32],
    offer: [u8; 32],
    owner: [u8; 32],
    acknowledgement: [u8; 32],
}
impl AuthorizedOwnerBiography {
    pub(crate) fn matches(&self, body: &str, root: &[u8; 32], offer: &[u8; 32]) -> bool {
        self.body == body && &self.root == root && &self.offer == offer
    }
    pub(crate) fn owner(&self) -> &[u8; 32] {
        &self.owner
    }
    pub(crate) fn acknowledgement(&self) -> &[u8; 32] {
        &self.acknowledgement
    }
}
impl Gate {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn authorize_biography(
        &mut self,
        request: &BiographyRequest,
        host: &str,
        boot: &str,
        body: &str,
        root: &[u8; 32],
        actual_offer: &[u8; 32],
        now_ms: u64,
    ) -> Result<AuthorizedOwnerBiography, &'static str> {
        let transcript = request.transcript()?;
        let c = &request.challenge;
        if c.host != host
            || c.boot != boot
            || c.body != body
            || &c.root_key != root
            || &c.offer != actual_offer
            || Some(c.nonce) != self.nonce
            || now_ms > self.deadline_ms
            || c.sequence <= self.last_sequence
        {
            return Err("biography-owner-scope");
        }
        let signature: [u8; 64] = request
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| "biography-owner-signature")?;
        self.key
            .verify_strict(&transcript, &Signature::from_bytes(&signature))
            .map_err(|_| "biography-owner-signature")?;
        self.nonce = None;
        self.last_sequence = c.sequence;
        Ok(AuthorizedOwnerBiography {
            body: c.body.clone(),
            root: *root,
            offer: *actual_offer,
            owner: Sha256::digest(self.key.to_bytes()).into(),
            acknowledgement: Sha256::digest(signature).into(),
        })
    }
}
