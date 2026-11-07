//! Optional transport authentication of an already committed admission receipt.
//! No function here admits a member, creates authority, or starts a Play.

use alloc::{string::String, vec::Vec};
use conduit_core::HostAdvertisement;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{BodyId, FederationPeer, PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA};

pub const SIGNED_ADMISSION_RECEIPT_SCHEMA: &str = "conduit.body/admission-receipt-attestation@1";
pub const MAX_ATTESTED_ADMISSION_RECEIPT_BYTES: usize = 16 * 1024;

/// Exact existing receipt bytes wrapped in independently pinned transport proof.
/// The embedded peer is descriptive: verification must compare an external pin.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPortableAdmissionReceipt {
    pub schema: String,
    pub receipt_json: String,
    pub owner: FederationPeer,
    pub member_verifying_key: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionReceiptAttestationRefusal {
    Bounds,
    Encoding,
    Owner,
    MemberKey,
    Signature,
    ReceiptBasis,
}

/// Sign only after the caller has checked the actual Core admission commit.
/// A signing closure keeps private owner key custody outside this document API.
pub fn attest_portable_admission_receipt(
    receipt: &PortableAdmissionReceipt,
    owner: FederationPeer,
    member_verifying_key: [u8; 32],
    sign: impl FnOnce(&[u8; 32]) -> [u8; 64],
) -> Result<SignedPortableAdmissionReceipt, AdmissionReceiptAttestationRefusal> {
    validate_attested_admission_receipt_basis(
        receipt,
        &receipt.credential.body_id,
        &receipt.host_advertisement,
    )?;
    let receipt_json =
        serde_json::to_string(receipt).map_err(|_| AdmissionReceiptAttestationRefusal::Encoding)?;
    attest_receipt_json(receipt_json, owner, member_verifying_key, sign)
}

pub(crate) fn attest_receipt_json(
    receipt_json: String,
    owner: FederationPeer,
    member_verifying_key: [u8; 32],
    sign: impl FnOnce(&[u8; 32]) -> [u8; 64],
) -> Result<SignedPortableAdmissionReceipt, AdmissionReceiptAttestationRefusal> {
    let transcript = admission_receipt_attestation_transcript(
        receipt_json.as_bytes(),
        owner.host_id.as_str(),
        owner.boot_id.as_str(),
        owner.offer_generation.0,
        &owner.verifying_key,
        &member_verifying_key,
    )?;
    Ok(SignedPortableAdmissionReceipt {
        schema: SIGNED_ADMISSION_RECEIPT_SCHEMA.into(),
        receipt_json,
        owner,
        member_verifying_key,
        signature: sign(&transcript).into(),
    })
}

/// Allocation-free transcript shared with constrained receivers. Hashes exact
/// receipt bytes; no alternate JSON encoding or identity can share the proof.
pub fn admission_receipt_attestation_transcript(
    receipt_json: &[u8],
    owner_host: &str,
    owner_boot: &str,
    owner_generation: u64,
    owner_key: &[u8; 32],
    member_key: &[u8; 32],
) -> Result<[u8; 32], AdmissionReceiptAttestationRefusal> {
    let mut transcript = crate::AdmissionReceiptTranscript::new(
        crate::AdmissionReceiptStreamBounds {
            receipt_bytes: receipt_json.len(),
            maximum_chunks: 1,
        },
        owner_host,
        owner_boot,
        owner_generation,
        owner_key,
        member_key,
    )?;
    transcript.push(receipt_json)?;
    transcript.finish()
}

/// Verify exact received receipt bytes against an independently pinned peer.
/// This allocation-free step authenticates transport evidence only; callers
/// must additionally validate the receipt's Body/Host/Boot/offer basis.
pub fn verify_admission_receipt_attestation(
    receipt_json: &[u8],
    expected_owner: &FederationPeer,
    member_key: &[u8; 32],
    signature: &[u8],
) -> Result<(), AdmissionReceiptAttestationRefusal> {
    let transcript = admission_receipt_attestation_transcript(
        receipt_json,
        expected_owner.host_id.as_str(),
        expected_owner.boot_id.as_str(),
        expected_owner.offer_generation.0,
        &expected_owner.verifying_key,
        member_key,
    )?;
    verify_receipt_transcript(&transcript, expected_owner, signature)
}

pub(crate) fn verify_receipt_transcript(
    transcript: &[u8; 32],
    expected_owner: &FederationPeer,
    signature: &[u8],
) -> Result<(), AdmissionReceiptAttestationRefusal> {
    let signature = Signature::from_slice(signature)
        .map_err(|_| AdmissionReceiptAttestationRefusal::Signature)?;
    VerifyingKey::from_bytes(&expected_owner.verifying_key)
        .map_err(|_| AdmissionReceiptAttestationRefusal::Owner)?
        .verify_strict(transcript, &signature)
        .map_err(|_| AdmissionReceiptAttestationRefusal::Signature)
}

impl SignedPortableAdmissionReceipt {
    /// Metadata frame for finite receivers; send `receipt_json.as_bytes()` as
    /// the separate exact receipt frame on the same selected transport.
    pub fn transport_metadata(
        &self,
    ) -> Result<crate::BorrowedAdmissionReceiptAttestation<'_>, AdmissionReceiptAttestationRefusal>
    {
        Ok(crate::BorrowedAdmissionReceiptAttestation {
            schema: &self.schema,
            owner: crate::BorrowedFederationPeer {
                host_id: self.owner.host_id.as_str(),
                boot_id: self.owner.boot_id.as_str(),
                offer_generation: self.owner.offer_generation.0,
                verifying_key: self.owner.verifying_key,
            },
            member_verifying_key: self.member_verifying_key,
            signature: self
                .signature
                .as_slice()
                .try_into()
                .map_err(|_| AdmissionReceiptAttestationRefusal::Signature)?,
        })
    }

    pub fn verify(
        &self,
        expected_owner: &FederationPeer,
        expected_member_key: &[u8; 32],
        expected_body: &BodyId,
        actual: &HostAdvertisement,
    ) -> Result<PortableAdmissionReceipt, AdmissionReceiptAttestationRefusal> {
        if self.schema != SIGNED_ADMISSION_RECEIPT_SCHEMA || &self.owner != expected_owner {
            return Err(AdmissionReceiptAttestationRefusal::Owner);
        }
        if &self.member_verifying_key != expected_member_key {
            return Err(AdmissionReceiptAttestationRefusal::MemberKey);
        }
        verify_admission_receipt_attestation(
            self.receipt_json.as_bytes(),
            expected_owner,
            expected_member_key,
            &self.signature,
        )?;
        let receipt = serde_json::from_str(&self.receipt_json)
            .map_err(|_| AdmissionReceiptAttestationRefusal::Encoding)?;
        validate_attested_admission_receipt_basis(&receipt, expected_body, actual)?;
        Ok(receipt)
    }
}

/// Check decoded canonical receipt identity and exact current offers after
/// transport authentication. This check grants no membership or effect authority.
pub fn validate_attested_admission_receipt_basis(
    receipt: &PortableAdmissionReceipt,
    body: &BodyId,
    actual: &HostAdvertisement,
) -> Result<(), AdmissionReceiptAttestationRefusal> {
    if receipt.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
        || !receipt.membership_admitted
        || receipt.plan_created
        || receipt.play_created
        || receipt.current_offers_available != !actual.capabilities.is_empty()
        || &receipt.host_advertisement != actual
        || &receipt.credential.body_id != body
        || receipt.credential.host_id != actual.host_id
        || receipt.credential.boot_id != actual.boot_id
    {
        return Err(AdmissionReceiptAttestationRefusal::ReceiptBasis);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MembershipCredential, MembershipCredentialId, PartId};
    use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};
    use ed25519_dalek::{Signer, SigningKey};

    fn fixture() -> (PortableAdmissionReceipt, FederationPeer, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let owner = FederationPeer {
            host_id: HostId::from("owner/physical"),
            boot_id: BootId::from("owner/boot"),
            offer_generation: OfferGeneration(1),
            verifying_key: key.verifying_key().to_bytes(),
        };
        let ad = HostAdvertisement {
            declared_name: None,
            protocol_version: 1,
            host_id: HostId::from("mcu/physical-flash-id"),
            boot_id: BootId::from("mcu/boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("mcu/identity-usb"),
            bases: Vec::new(),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        let receipt = PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: MembershipCredential {
                credential_id: MembershipCredentialId::bound("credential/1".into()),
                body_id: BodyId::bound("house/1".into()),
                part_id: PartId::bound("part/green".into()),
                host_id: ad.host_id.clone(),
                boot_id: ad.boot_id.clone(),
                issued_at_millis: 10,
            },
            host_advertisement: ad,
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        };
        (receipt, owner, key)
    }

    #[test]
    fn authenticated_receipt_binds_pin_member_body_boot_and_exact_offers() {
        let (receipt, owner, key) = fixture();
        let member_key = [9; 32];
        let signed = attest_portable_admission_receipt(&receipt, owner.clone(), member_key, |t| {
            key.sign(t).to_bytes()
        })
        .unwrap();
        let verify = |s: &SignedPortableAdmissionReceipt| {
            s.verify(
                &owner,
                &member_key,
                &receipt.credential.body_id,
                &receipt.host_advertisement,
            )
        };
        assert!(verify(&signed).is_ok());
        let metadata_bytes = serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap();
        let metadata: crate::BorrowedAdmissionReceiptAttestation<'_> =
            serde_json::from_slice(&metadata_bytes).unwrap();
        assert!(metadata
            .verify(signed.receipt_json.as_bytes(), &owner, &member_key)
            .is_ok());
        assert!(metadata.verify(b"{}", &owner, &member_key).is_err());

        let mut changed = signed.clone();
        changed.receipt_json.push(' ');
        assert!(matches!(
            verify(&changed),
            Err(AdmissionReceiptAttestationRefusal::Signature)
        ));
        let mut changed = signed.clone();
        changed.owner.boot_id = BootId::from("owner/other-boot");
        assert!(matches!(
            verify(&changed),
            Err(AdmissionReceiptAttestationRefusal::Owner)
        ));
        assert!(signed
            .verify(
                &owner,
                &[8; 32],
                &receipt.credential.body_id,
                &receipt.host_advertisement
            )
            .is_err());
        assert!(signed
            .verify(
                &owner,
                &member_key,
                &BodyId::bound("house/other".into()),
                &receipt.host_advertisement
            )
            .is_err());
        let mut actual = receipt.host_advertisement.clone();
        actual.offer_generation = OfferGeneration(2);
        assert!(signed
            .verify(&owner, &member_key, &receipt.credential.body_id, &actual)
            .is_err());
        actual = receipt.host_advertisement.clone();
        actual.boot_id = BootId::from("mcu/next-boot");
        assert!(signed
            .verify(&owner, &member_key, &receipt.credential.body_id, &actual)
            .is_err());
    }

    #[test]
    fn attestation_refuses_execution_claim_before_signing() {
        let (mut receipt, owner, _) = fixture();
        receipt.play_created = true;
        assert!(
            attest_portable_admission_receipt(&receipt, owner, [9; 32], |_| panic!(
                "invalid receipt must not reach signer"
            ))
            .is_err()
        );
    }
}
