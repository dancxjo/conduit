//! Compact return evidence for externally resolved immutable offer images.
//! This projection is not a HostAdvertisement until its images are resolved.
use crate::{
    AdmissionReceiptAttestationRefusal as R, FederationPeer, MemberReturnAcknowledgement,
    MembershipCredential, SignedPortableAdmissionReceipt, MAX_CANDIDATE_PLANNER_CAPABILITIES,
    MAX_CANDIDATE_RESOURCES,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{AuthorityGrant, HostAdvertisement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MEMBER_RETURN_IMAGES_SCHEMA: &str = "conduit.body/member-return-images@2";
pub const PICO_MEMBER_RETURN_IMAGES_REQUEST: &[u8] = b"CONDUIT_MEMBER_RETURN_IMAGES@3";
/// This carriage profile is deliberately smaller than general Host catalogs.
// Finite discovery carriage; grants and execution bounds are admitted separately.
pub const MAX_MEMBER_RETURN_IMAGES: usize = 32;
/// At most one descriptive grant per general candidate capability. The smaller
/// aggregate text ceiling independently bounds this compact carriage profile.
pub use crate::authority_grant_descriptions::{
    MAX_MEMBER_RETURN_IMAGE_GRANTS, MAX_MEMBER_RETURN_IMAGE_GRANT_ID_BYTES,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostAdvertisementImageProjection {
    /// Exact non-capability fields. Capabilities MUST be empty here; they are
    /// represented only by the ordered image references below.
    pub context: HostAdvertisement,
    pub capability_images: Vec<[u8; 32]>,
    /// Original independently issued grants, not inferred from resolved images.
    /// Attribution does not establish freshness or opaque native possession.
    pub authority_grants: Vec<AuthorityGrant>,
}
/// A view of the original native descriptions; constructing this view neither
/// copies descriptions nor issues native authority.
#[derive(Clone, Copy, Serialize)]
pub struct BorrowedHostAdvertisementImageProjection<'a> {
    pub context: &'a HostAdvertisement,
    pub capability_images: &'a [[u8; 32]],
    pub authority_grants: &'a [AuthorityGrant],
}
impl BorrowedHostAdvertisementImageProjection<'_> {
    pub fn validate(&self) -> Result<(), R> {
        if !self.context.capabilities.is_empty()
            || self.context.resources.len() > MAX_CANDIDATE_RESOURCES
            || self.context.planner_capabilities.len() > MAX_CANDIDATE_PLANNER_CAPABILITIES
            || self.capability_images.len() > MAX_MEMBER_RETURN_IMAGES
            || self
                .capability_images
                .iter()
                .enumerate()
                .any(|(i, hash)| self.capability_images[..i].contains(hash))
        {
            return Err(R::ReceiptBasis);
        }
        crate::authority_grant_descriptions::validate(self.context, self.authority_grants)
            .map_err(|_| R::ReceiptBasis)
    }
}

/// The exact existing signature transcript, encoded into caller-funded storage.
/// The scratch can be reused for the subsequent presentation and acknowledgement.
pub fn member_return_image_transcript_into(
    credential: &MembershipCredential,
    projection: BorrowedHostAdvertisementImageProjection<'_>,
    scratch: &mut [u8],
) -> Result<[u8; 32], R> {
    projection.validate()?;
    let used =
        serde_json_core::to_slice(&(credential, projection), scratch).map_err(|_| R::Encoding)?;
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/member-return-image-presentation@2");
    hash.update(&scratch[..used]);
    Ok(hash.finalize().into())
}

impl HostAdvertisementImageProjection {
    pub fn borrowed(&self) -> BorrowedHostAdvertisementImageProjection<'_> {
        BorrowedHostAdvertisementImageProjection {
            context: &self.context,
            capability_images: &self.capability_images,
            authority_grants: &self.authority_grants,
        }
    }
    pub fn validate(&self) -> Result<(), R> {
        self.borrowed().validate()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberReturnImagePresentation {
    pub signature: Vec<u8>,
    pub kind: String,
    pub schema: String,
    pub credential: MembershipCredential,
    pub projection: HostAdvertisementImageProjection,
}

/// Attribute the entire projection, including every image reference and context
/// field, to the returning member. The ordinary fresh challenge remains required.
pub fn member_return_image_transcript(
    credential: &MembershipCredential,
    projection: &HostAdvertisementImageProjection,
) -> Result<[u8; 32], R> {
    projection.validate()?;
    let encoded = serde_json::to_vec(&(credential, projection)).map_err(|_| R::Encoding)?;
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/member-return-image-presentation@2");
    hash.update(encoded);
    Ok(hash.finalize().into())
}
impl MemberReturnImagePresentation {
    pub fn verify(&self, member_key: &[u8; 32]) -> Result<(), R> {
        if self.kind != "Advertise"
            || self.schema != MEMBER_RETURN_IMAGES_SCHEMA
            || self.credential.host_id != self.projection.context.host_id
        {
            return Err(R::ReceiptBasis);
        }
        let signature =
            ed25519_dalek::Signature::from_slice(&self.signature).map_err(|_| R::Signature)?;
        ed25519_dalek::VerifyingKey::from_bytes(member_key)
            .map_err(|_| R::MemberKey)?
            .verify_strict(
                &member_return_image_transcript(&self.credential, &self.projection)?,
                &signature,
            )
            .map_err(|_| R::Signature)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberReturnImageAcknowledgement {
    pub kind: String,
    pub schema: String,
    pub credential: MembershipCredential,
    pub projection: HostAdvertisementImageProjection,
}
impl MemberReturnImageAcknowledgement {
    pub fn new(
        credential: MembershipCredential,
        projection: HostAdvertisementImageProjection,
    ) -> Self {
        Self {
            kind: "Admitted".into(),
            schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
            credential,
            projection,
        }
    }
    /// Compare the exact locally retained projection after checking the pinned
    /// signature. This does not decode offers or grant execution authority.
    pub fn validate_for(
        &self,
        retained: &MembershipCredential,
        actual: &HostAdvertisementImageProjection,
    ) -> Result<(), R> {
        actual.validate()?;
        if self.kind != "Admitted"
            || self.schema != MEMBER_RETURN_IMAGES_SCHEMA
            || self.projection != *actual
        {
            return Err(R::ReceiptBasis);
        }
        // Share existing durable relationship/current-incarnation laws. The
        // context is small; full contract metadata is never cloned here.
        MemberReturnAcknowledgement::new(self.credential.clone(), actual.context.clone())
            .validate_for(retained, &actual.context)
    }
}

/// Caller must first resolve the images and check the actual Core commit. This
/// operation only authenticates the compact acknowledgement's exact bytes.
pub fn attest_member_return_image_acknowledgement(
    receipt: &MemberReturnImageAcknowledgement,
    owner: FederationPeer,
    member_key: [u8; 32],
    sign: impl FnOnce(&[u8; 32]) -> [u8; 64],
) -> Result<SignedPortableAdmissionReceipt, R> {
    receipt.validate_for(&receipt.credential, &receipt.projection)?;
    crate::admission_receipt_attestation::attest_receipt_json(
        serde_json::to_string(receipt).map_err(|_| R::Encoding)?,
        owner,
        member_key,
        sign,
    )
}
impl SignedPortableAdmissionReceipt {
    pub fn verify_image_return(
        &self,
        owner: &FederationPeer,
        member_key: &[u8; 32],
        retained: &MembershipCredential,
        actual: &HostAdvertisementImageProjection,
    ) -> Result<MemberReturnImageAcknowledgement, R> {
        self.transport_metadata()?
            .verify(self.receipt_json.as_bytes(), owner, member_key)?;
        let receipt: MemberReturnImageAcknowledgement =
            serde_json::from_str(&self.receipt_json).map_err(|_| R::Encoding)?;
        receipt.validate_for(retained, actual)?;
        Ok(receipt)
    }
}

#[cfg(test)]
#[path = "member_return_images_tests.rs"]
mod tests;

#[path = "member_return_images_borrowed.rs"]
mod borrowed;
pub use borrowed::MemberReturnImageAcknowledgementSeed;
