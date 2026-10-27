//! Opaque proof of an exact, authenticated epoch retained in durable host files.
//! The binding owner supplies the independently pinned Body and Root. This proof
//! permits only a typed custody signature, never an arbitrary signing operation.
use crate::{checked, observed_history::Collector, Result};
use conduit_body::{BodyId, MembershipCredential};
use sha2::{Digest, Sha256};
use std::path::Path;
#[allow(clippy::duplicate_mod)] // Same exact portable wire as the native verifier.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_custody_wire.rs"]
pub(crate) mod wire;

pub(crate) struct VerifiedDurableEpoch {
    root: [u8; 32],
    body: [u8; 32],
    offer: [u8; 32],
}
impl VerifiedDurableEpoch {
    pub(crate) fn verify(
        offer: &wire::Offer<'_>,
        body: &BodyId,
        root: &[u8; 32],
        directory: &Path,
    ) -> Result<Self> {
        let key = checked(ed25519_dalek::VerifyingKey::from_bytes(root))?;
        let verified = checked(wire::verify_offer(offer, body, &key))?;
        let collector = Collector::from_authenticated_scope(*verified.scope(), root)?;
        let expected = (verified.count(), *verified.digest());
        if collector.durable_summary(directory, verified.forms())? != expected {
            return Err("custody files do not cover exact closed epoch".into());
        }
        // Reopen through a new collector after the fsync boundary. The proof is
        // not minted from a cached cursor or an in-memory successful append.
        let reopened = Collector::from_authenticated_scope(*verified.scope(), root)?;
        if reopened.durable_summary(directory, verified.forms())? != expected {
            return Err("custody files changed after durable verification".into());
        }
        Ok(Self {
            root: *root,
            body: Sha256::digest(body.as_str().as_bytes()).into(),
            offer: verified.id(),
        })
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn body_digest(&self) -> &[u8; 32] {
        &self.body
    }
    pub(crate) fn offer_hash(&self) -> &[u8; 32] {
        &self.offer
    }
    fn request<'a>(&self, credential: &'a MembershipCredential) -> Result<wire::Request<'a>> {
        if Sha256::digest(credential.body_id.as_str().as_bytes())[..] != self.body {
            return Err("custody credential Body differs".into());
        }
        let request = wire::Request {
            schema: wire::ACK,
            offer: wire::Hex(self.offer),
            part: credential.part_id.as_str(),
            host: credential.host_id.as_str(),
            boot: credential.boot_id.as_str(),
            signature: wire::Hex([0; 64]),
        };
        checked(request.validate())?;
        Ok(request)
    }
    pub(crate) fn signing_transcript(&self, credential: &MembershipCredential) -> Result<[u8; 32]> {
        Ok(self.request(credential)?.transcript())
    }
    pub(crate) fn encode_ack(
        &self,
        credential: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<Vec<u8>> {
        let mut request = self.request(credential)?;
        request.signature = wire::Hex(signature);
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() > wire::MAX_REQUEST {
            return Err("custody request frame".into());
        }
        Ok(bytes)
    }
}
