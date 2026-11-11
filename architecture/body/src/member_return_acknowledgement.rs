//! Canonical member-return acknowledgement and optional pinned transport proof.
//! Descriptive receipt bytes never replace the owner's durable Core admission.
use crate::{
    AdmissionReceiptAttestationRefusal as Refusal, FederationPeer, MembershipCredential,
    SignedPortableAdmissionReceipt, MAX_LIFECYCLE_ID_BYTES,
};
use alloc::string::String;
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

pub const MEMBER_RETURN_SCHEMA: &str = "conduit.body/member-return@1";
/// The same `Admitted` document carried by the native TLS member-return route.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberReturnAcknowledgement {
    pub kind: String,
    pub schema: String,
    pub credential: MembershipCredential,
    pub advertisement: HostAdvertisement,
}
impl MemberReturnAcknowledgement {
    pub fn new(credential: MembershipCredential, advertisement: HostAdvertisement) -> Self {
        Self {
            kind: "Admitted".into(),
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential,
            advertisement,
        }
    }
    /// Exact retained relationship and actual current incarnation, after checking
    /// the independently pinned signature. No offer or effect grant is implied.
    pub fn validate_for(
        &self,
        retained: &MembershipCredential,
        actual: &HostAdvertisement,
    ) -> Result<(), Refusal> {
        if self.kind != "Admitted"
            || self.schema != MEMBER_RETURN_SCHEMA
            || self.advertisement != *actual
        {
            return Err(Refusal::ReceiptBasis);
        }
        validate_relationship(&self.credential, retained, actual)
    }
}

pub(crate) fn validate_relationship(
    credential: &MembershipCredential,
    retained: &MembershipCredential,
    actual: &HostAdvertisement,
) -> Result<(), Refusal> {
    let finite = |id: &str| !id.is_empty() && id.len() <= MAX_LIFECYCLE_ID_BYTES;
    if credential.body_id != retained.body_id
        || credential.part_id != retained.part_id
        || credential.host_id != retained.host_id
        || credential.host_id != actual.host_id
        || credential.boot_id != actual.boot_id
        || actual.protocol_version != PROTOCOL_VERSION
        || actual.offer_generation.0 == 0
        || ![
            credential.body_id.as_str(),
            credential.part_id.as_str(),
            credential.credential_id.as_str(),
            actual.host_id.as_str(),
            actual.boot_id.as_str(),
            actual.profile.as_str(),
        ]
        .into_iter()
        .all(finite)
    {
        return Err(Refusal::ReceiptBasis);
    }
    Ok(())
}
/// Caller must check committed Core state; this function authenticates bytes only.
pub fn attest_member_return_acknowledgement(
    receipt: &MemberReturnAcknowledgement,
    owner: FederationPeer,
    member_key: [u8; 32],
    sign: impl FnOnce(&[u8; 32]) -> [u8; 64],
) -> Result<SignedPortableAdmissionReceipt, Refusal> {
    receipt.validate_for(&receipt.credential, &receipt.advertisement)?;
    crate::admission_receipt_attestation::attest_receipt_json(
        serde_json::to_string(receipt).map_err(|_| Refusal::Encoding)?,
        owner,
        member_key,
        sign,
    )
}
impl SignedPortableAdmissionReceipt {
    /// Return and spawn remain different receipt semantics, sharing only the
    /// existing exact-byte attestation and finite metadata transport.
    pub fn verify_return(
        &self,
        owner: &FederationPeer,
        member_key: &[u8; 32],
        retained: &MembershipCredential,
        actual: &HostAdvertisement,
    ) -> Result<MemberReturnAcknowledgement, Refusal> {
        self.transport_metadata()?
            .verify(self.receipt_json.as_bytes(), owner, member_key)?;
        let receipt: MemberReturnAcknowledgement =
            serde_json::from_str(&self.receipt_json).map_err(|_| Refusal::Encoding)?;
        receipt.validate_for(retained, actual)?;
        Ok(receipt)
    }
}
#[cfg(test)]
mod tests;
