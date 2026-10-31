//! Explicit membership-only documents, separate from initialized equipment.
//! Decoding grants nothing. Only ordinary invitation completion admits membership;
//! receipt signatures are meaningful against an independently trusted Root pin.
use super::{MembershipCredential, SpawnAdmissionHostScope, SpawnAdmissionProof};
use crate::{identity::validate_ids, BodyId, SpawnInvitationId};
use alloc::string::String;
use conduit_core::{BootId, HostId, OfferGeneration, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MEMBERSHIP_ADMISSION_REQUEST_SCHEMA: &str = "conduit.body/membership-admission-request@1";
pub const MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA: &str = "conduit.body/membership-admission-receipt@1";
pub const MAX_MEMBERSHIP_ADMISSION_REQUEST_BYTES: usize = 1024;
pub const MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipAdmissionDocumentRefusal {
    Capacity,
    Encoding,
    Schema,
    Protocol,
    Identity,
    Generation,
    Nonce,
    ReceiptBasis,
    RootKey,
    Signature,
}

/// Identity/proof carrier, never an empty or restricted worker advertisement.
/// Exact equipment and offer eligibility must be established separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableMembershipAdmissionRequest {
    pub schema: String,
    pub protocol_version: u16,
    pub invitation_id: SpawnInvitationId,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub nonce: [u8; 32],
    #[serde(with = "signature_bytes")]
    pub signature: [u8; 64],
}
impl PortableMembershipAdmissionRequest {
    /// Bound the entire input before any owned JSON field can be allocated.
    pub fn decode(bytes: &[u8]) -> Result<Self, MembershipAdmissionDocumentRefusal> {
        bound(bytes, MAX_MEMBERSHIP_ADMISSION_REQUEST_BYTES)?;
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|_| MembershipAdmissionDocumentRefusal::Encoding)?;
        value.validate()?;
        Ok(value)
    }
    /// Document consistency only; signature/expiry/replay remain manager checks.
    pub fn validate(&self) -> Result<(), MembershipAdmissionDocumentRefusal> {
        use MembershipAdmissionDocumentRefusal as E;
        if self.schema != MEMBERSHIP_ADMISSION_REQUEST_SCHEMA {
            return Err(E::Schema);
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(E::Protocol);
        }
        validate_ids(&[
            self.invitation_id.as_str(),
            self.body_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
        ])
        .map_err(|_| E::Identity)?;
        if self.offer_generation.0 == 0 {
            return Err(E::Generation);
        }
        if self.nonce == [0; 32] {
            return Err(E::Nonce);
        }
        Ok(())
    }
    /// Exact canonical request identity without JSON allocation. The domain
    /// fixes this profile's schema; callers validate the schema before use.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/membership-admission-request@1\0");
        hash.update(self.protocol_version.to_le_bytes());
        for value in [
            self.invitation_id.as_str(),
            self.body_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.offer_generation.0.to_le_bytes());
        hash.update(self.nonce);
        hash.update(self.signature);
        hash.finalize().into()
    }
    /// Borrow description only. The completion entrance still verifies proof.
    pub fn host_scope(&self) -> SpawnAdmissionHostScope<'_> {
        SpawnAdmissionHostScope::new(&self.host_id, &self.boot_id, self.offer_generation)
    }
    pub fn admission_proof(
        &self,
    ) -> Result<SpawnAdmissionProof, MembershipAdmissionDocumentRefusal> {
        self.validate()?;
        Ok(SpawnAdmissionProof {
            invitation_id: self.invitation_id.clone(),
            body_id: self.body_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            nonce: self.nonce,
            signature: self.signature,
        })
    }
}

/// Root-endorsed evidence of membership only. A public constructor or decoder
/// does not authenticate the signer or establish durable Root publication.
/// The actual Root signs only after canonical admission and durable checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableMembershipAdmissionReceipt {
    pub schema: String,
    pub protocol_version: u16,
    pub credential: MembershipCredential,
    pub membership_revision: u64,
    pub request_digest: [u8; 32],
    pub producer_host: HostId,
    pub producer_boot: BootId,
    pub producer_key: [u8; 32],
    pub invitation_id: SpawnInvitationId,
    pub offer_generation: OfferGeneration,
    pub membership_admitted: bool,
    pub current_offers_available: bool,
    pub plan_created: bool,
    pub play_created: bool,
    #[serde(with = "signature_bytes")]
    pub signature: [u8; 64],
}
impl PortableMembershipAdmissionReceipt {
    pub fn decode(bytes: &[u8]) -> Result<Self, MembershipAdmissionDocumentRefusal> {
        bound(bytes, MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES)?;
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|_| MembershipAdmissionDocumentRefusal::Encoding)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), MembershipAdmissionDocumentRefusal> {
        use MembershipAdmissionDocumentRefusal as E;
        if self.schema != MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA {
            return Err(E::Schema);
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(E::Protocol);
        }
        validate_ids(&[
            self.credential.credential_id.as_str(),
            self.credential.body_id.as_str(),
            self.credential.part_id.as_str(),
            self.credential.host_id.as_str(),
            self.credential.boot_id.as_str(),
            self.producer_host.as_str(),
            self.producer_boot.as_str(),
            self.invitation_id.as_str(),
        ])
        .map_err(|_| E::Identity)?;
        if self.offer_generation.0 == 0 {
            return Err(E::Generation);
        }
        if self.membership_revision == 0
            || !self.membership_admitted
            || self.current_offers_available
            || self.plan_created
            || self.play_created
        {
            return Err(E::ReceiptBasis);
        }
        Ok(())
    }
    /// Correlation only: callers must separately verify the trusted Root signature.
    pub fn validate_against(
        &self,
        request: &PortableMembershipAdmissionRequest,
    ) -> Result<(), MembershipAdmissionDocumentRefusal> {
        self.validate()?;
        request.validate()?;
        if self.request_digest != request.digest()
            || self.invitation_id != request.invitation_id
            || self.offer_generation != request.offer_generation
            || self.credential.body_id != request.body_id
            || self.credential.host_id != request.host_id
            || self.credential.boot_id != request.boot_id
        {
            return Err(MembershipAdmissionDocumentRefusal::ReceiptBasis);
        }
        Ok(())
    }
    /// Canonical public receipt transcript; possession of these bytes is not
    /// possession of the Root signing key or an execution grant.
    pub fn transcript(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"conduit.body/membership-admission-receipt@1\0");
        h.update(self.protocol_version.to_le_bytes());
        h.update(self.request_digest);
        for value in [
            self.credential.credential_id.as_str(),
            self.credential.body_id.as_str(),
            self.credential.part_id.as_str(),
            self.credential.host_id.as_str(),
            self.credential.boot_id.as_str(),
            self.producer_host.as_str(),
            self.producer_boot.as_str(),
            self.invitation_id.as_str(),
        ] {
            h.update((value.len() as u64).to_le_bytes());
            h.update(value.as_bytes());
        }
        h.update(self.credential.issued_at_millis.to_le_bytes());
        h.update(self.membership_revision.to_le_bytes());
        h.update(self.offer_generation.0.to_le_bytes());
        h.update(self.producer_key);
        h.update([
            self.membership_admitted as u8,
            self.current_offers_available as u8,
            self.plan_created as u8,
            self.play_created as u8,
        ]);
        h.finalize().into()
    }
    pub fn verify(
        &self,
        expected_root: &[u8; 32],
        request: &PortableMembershipAdmissionRequest,
    ) -> Result<(), MembershipAdmissionDocumentRefusal> {
        use MembershipAdmissionDocumentRefusal as E;
        self.validate_against(request)?;
        if &self.producer_key != expected_root {
            return Err(E::RootKey);
        }
        ed25519_dalek::VerifyingKey::from_bytes(expected_root)
            .map_err(|_| E::RootKey)?
            .verify_strict(
                &self.transcript(),
                &ed25519_dalek::Signature::from_bytes(&self.signature),
            )
            .map_err(|_| E::Signature)
    }
}
fn bound(bytes: &[u8], maximum: usize) -> Result<(), MembershipAdmissionDocumentRefusal> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(MembershipAdmissionDocumentRefusal::Capacity);
    }
    Ok(())
}

// A fixed array avoids collecting an attacker-controlled signature Vec.
mod signature_bytes {
    use core::fmt;
    use serde::{
        de::{Error, SeqAccess, Visitor},
        Deserializer, Serialize, Serializer,
    };
    pub(super) fn serialize<S: Serializer>(
        bytes: &[u8; 64],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        bytes.as_slice().serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; 64], D::Error> {
        struct Fixed;
        impl<'de> Visitor<'de> for Fixed {
            type Value = [u8; 64];
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("exactly 64 signature bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut out = [0; 64];
                for byte in &mut out {
                    *byte = sequence
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("signature too short"))?;
                }
                if sequence.next_element::<u8>()?.is_some() {
                    return Err(A::Error::custom("signature too long"));
                }
                Ok(out)
            }
        }
        // Fixed-size tuple is accepted by ordinary JSON and serde_json_core.
        deserializer.deserialize_tuple(64, Fixed)
    }
}
