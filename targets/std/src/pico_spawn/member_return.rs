//! USB carriage of canonical retained Part return, never another invitation.
use super::{PicoSpawnSocket, PicoSpawnTransportError as Error};
use conduit_body::{
    FederationPeer, MemberReturnImageAcknowledgement, MemberReturnProofDocument,
    MembershipCredential, PartReturnChallenge, PartReturnProof, SignedPortableAdmissionReceipt,
    MAX_PICO_ADMISSION_FRAME_BYTES, MEMBER_RETURN_IMAGES_SCHEMA, PICO_MEMBER_RETURN_IMAGES_REQUEST,
};
use conduit_core::{HostAdvertisement, HostProfileId, PROTOCOL_VERSION};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicoMemberReturnBasis {
    pub credential: MembershipCredential,
    pub expected_profile: HostProfileId,
    pub member_verifying_key: [u8; 32],
    pub owner: FederationPeer,
}
pub type MemberReturnPresentation = conduit_body::MemberReturnImagePresentation;
pub struct MemberReturnObservation {
    pub presentation: MemberReturnPresentation,
    pub images: Vec<Vec<u8>>,
    pub advertisement: HostAdvertisement,
}
impl PicoMemberReturnBasis {
    pub fn validate(&self) -> Result<(), Error> {
        for id in [
            self.credential.credential_id.as_str(),
            self.credential.body_id.as_str(),
            self.credential.part_id.as_str(),
            self.credential.host_id.as_str(),
            self.credential.boot_id.as_str(),
            self.expected_profile.as_str(),
            self.owner.host_id.as_str(),
            self.owner.boot_id.as_str(),
        ] {
            if id.is_empty()
                || id.len() > conduit_body::MAX_LIFECYCLE_ID_BYTES
                || id.chars().any(char::is_control)
            {
                return Err(Error::Malformed);
            }
        }
        if self.owner.offer_generation.0 == 0 {
            return Err(Error::Malformed);
        }
        for key in [&self.member_verifying_key, &self.owner.verifying_key] {
            if *key == [0; 32] {
                return Err(Error::Malformed);
            }
            VerifyingKey::from_bytes(key).map_err(|_| Error::Malformed)?;
        }
        Ok(())
    }
    /// Advance only the credential using an original-owner-attested receipt and
    /// an independently observed exact current image projection. This verifies
    /// the same durable relationship, full projection and pinned owner/member
    /// keys as receipt delivery; it creates no membership, recovery permission,
    /// latest-Core-state proof or live carrier evidence. Normal presentations
    /// still require exact credential equality against the returned basis.
    pub fn advance_from_verified_receipt(
        &self,
        signed: &SignedPortableAdmissionReceipt,
        actual_projection: &conduit_body::HostAdvertisementImageProjection,
    ) -> Result<Self, Error> {
        self.validate()?;
        if actual_projection.context.profile != self.expected_profile {
            return Err(Error::WrongAdvertisement);
        }
        let acknowledged = signed
            .verify_image_return(
                &self.owner,
                &self.member_verifying_key,
                &self.credential,
                actual_projection,
            )
            .map_err(|_| Error::InvalidReceipt)?;
        let mut advanced = self.clone();
        advanced.credential = acknowledged.credential;
        advanced.validate()?;
        Ok(advanced)
    }
    pub fn check_presentation(&self, claim: &MemberReturnPresentation) -> Result<(), Error> {
        self.validate()?;
        claim
            .verify(&self.member_verifying_key)
            .map_err(|_| Error::InvalidReceipt)?;
        claim
            .projection
            .validate()
            .map_err(|_| Error::WrongAdvertisement)?;
        let ad = &claim.projection.context;
        if claim.schema != MEMBER_RETURN_IMAGES_SCHEMA
            || claim.kind != "Advertise"
            || claim.credential != self.credential
            || ad.host_id != self.credential.host_id
            || ad.profile != self.expected_profile
            || ad.protocol_version != PROTOCOL_VERSION
            || ad.offer_generation.0 == 0
            || ad.boot_id.as_str().is_empty()
            || ad.boot_id.as_str().len() > conduit_body::MAX_LIFECYCLE_ID_BYTES
        {
            return Err(Error::WrongAdvertisement);
        }
        Ok(())
    }
    pub fn check_proof(
        &self,
        actual: &HostAdvertisement,
        challenge: &PartReturnChallenge,
        proof: &PartReturnProof,
    ) -> Result<(), Error> {
        if challenge.body_id != self.credential.body_id
            || challenge.part_id != self.credential.part_id
            || challenge.host_id != self.credential.host_id
            || challenge.host_id != actual.host_id
            || challenge.boot_id != actual.boot_id
            || challenge.offer_generation != actual.offer_generation
            || challenge.nonce == [0; 32]
            || challenge.expires_at_millis <= challenge.issued_at_millis
            || proof.admission_id != challenge.admission_id
            || proof.body_id != challenge.body_id
            || proof.part_id != challenge.part_id
            || proof.host_id != challenge.host_id
            || proof.boot_id != challenge.boot_id
            || proof.nonce != challenge.nonce
        {
            return Err(Error::WrongAdvertisement);
        }
        VerifyingKey::from_bytes(&self.member_verifying_key)
            .map_err(|_| Error::Malformed)?
            .verify_strict(
                &challenge.signing_transcript(),
                &Signature::from_bytes(&proof.signature),
            )
            .map_err(|_| Error::InvalidReceipt)
    }
}
impl PicoSpawnSocket {
    pub fn request_member_return(
        &mut self,
        basis: &PicoMemberReturnBasis,
        timeout: Duration,
    ) -> Result<MemberReturnObservation, Error> {
        basis.validate()?;
        self.line
            .send_raw_stream_frame(PICO_MEMBER_RETURN_IMAGES_REQUEST, timeout)?;
        let mut bytes = [0; MAX_PICO_ADMISSION_FRAME_BYTES];
        let mut document = conduit_wire::BoundedDocumentAssembly::new(16 * 1024, 32);
        loop {
            let frame = self.line.receive_raw_stream_frame(&mut bytes, timeout)?;
            if document.push(frame).map_err(|_| Error::Malformed)? {
                break;
            }
        }
        let presentation = serde_json::from_slice(document.completed().ok_or(Error::Malformed)?.0)
            .map_err(|_| Error::Malformed)?;
        basis.check_presentation(&presentation)?;
        let images = self.receive_offer_images(&presentation.projection, timeout)?;
        let advertisement =
            super::offer_images::resolve_member_offer_images(&presentation.projection, &images)?;
        Ok(MemberReturnObservation {
            presentation,
            images,
            advertisement,
        })
    }
    pub fn prove_member_return(
        &mut self,
        basis: &PicoMemberReturnBasis,
        actual: &HostAdvertisement,
        challenge: &PartReturnChallenge,
        timeout: Duration,
    ) -> Result<PartReturnProof, Error> {
        let bytes = serde_json::to_vec(challenge).map_err(|_| Error::Malformed)?;
        if bytes.len() > MAX_PICO_ADMISSION_FRAME_BYTES {
            return Err(Error::Oversized);
        }
        self.line.send_raw_stream_frame(&bytes, timeout)?;
        let mut bytes = [0; MAX_PICO_ADMISSION_FRAME_BYTES];
        let proof: MemberReturnProofDocument =
            serde_json::from_slice(self.line.receive_raw_stream_frame(&mut bytes, timeout)?)
                .map_err(|_| Error::Malformed)?;
        let proof = proof.decode().map_err(|_| Error::Malformed)?;
        basis.check_proof(actual, challenge, &proof)?;
        Ok(proof)
    }
    pub fn deliver_member_return(
        &mut self,
        signed: &SignedPortableAdmissionReceipt,
        basis: &PicoMemberReturnBasis,
        actual: &conduit_body::HostAdvertisementImageProjection,
        timeout: Duration,
    ) -> Result<MemberReturnImageAcknowledgement, Error> {
        let ack = signed
            .verify_image_return(
                &basis.owner,
                &basis.member_verifying_key,
                &basis.credential,
                actual,
            )
            .map_err(|_| Error::InvalidReceipt)?;
        let metadata = serde_json::to_vec(
            &signed
                .transport_metadata()
                .map_err(|_| Error::InvalidReceipt)?,
        )
        .map_err(|_| Error::Malformed)?;
        if metadata.len() > MAX_PICO_ADMISSION_FRAME_BYTES || signed.receipt_json.len() > 16 * 1024
        {
            return Err(Error::Oversized);
        }
        self.line.send_raw_stream_frame(&metadata, timeout)?;
        let document = signed.receipt_json.as_bytes();
        let digest = sha2::Sha256::digest(document).into();
        let mut frame = [0; MAX_PICO_ADMISSION_FRAME_BYTES];
        let mut offset = 0;
        while offset < document.len() {
            let (length, next) =
                conduit_wire::encode_bounded_document_chunk(&mut frame, document, offset, &digest)
                    .map_err(|_| Error::Malformed)?;
            self.line.send_raw_stream_frame(&frame[..length], timeout)?;
            offset = next;
        }
        Ok(ack)
    }
}
#[cfg(test)]
mod tests;
