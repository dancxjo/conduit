//! Canonical retained-Part return, independent of physical carrier or GPIO.
extern crate alloc;
use crate::{membership::AcceptedMembership, transport::DocumentTransport};
use alloc::vec::Vec;
use conduit_body::{
    BorrowedAdmissionReceiptAttestation, BorrowedHostAdvertisementImageProjection,
    BorrowedPartReturnChallenge, BorrowedPartReturnProof, MemberBootstrap,
    MemberReturnImageAcknowledgementSeed, MembershipCredential, MEMBER_RETURN_IMAGES_SCHEMA,
};
use conduit_core::{AuthorityGrant, HostAdvertisement};
use ed25519_dalek::{Signer, SigningKey};
use serde::{de::DeserializeSeed, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    Bounds,
    Encoding,
    Transport,
    Challenge,
    Attestation,
    Receipt,
}
#[derive(Serialize)]
struct Presentation<'a> {
    kind: &'static str,
    schema: &'static str,
    credential: &'a MembershipCredential,
    projection: BorrowedHostAdvertisementImageProjection<'a>,
    signature: &'a [u8],
}
/// Root retains native publication/authorization custody before this exchange.
/// A projected grant is descriptive attribution, never an issued native token.
pub struct RetainedReturn<'a, 's> {
    pub bootstrap: &'a MemberBootstrap<'s>,
    pub actual: &'a HostAdvertisement,
    /// Independently supplied current running IMAGE, not read from bootstrap.
    pub image_id: &'a str,
    pub key: &'a SigningKey,
    pub retained: &'a MembershipCredential,
}
impl RetainedReturn<'_, '_> {
    pub async fn exchange<'i, T, I, G>(
        &self,
        link: &mut T,
        images: I,
        authority_grants: G,
        input: &mut [u8],
        output: &mut [u8],
    ) -> Result<AcceptedMembership, Refusal>
    where
        T: DocumentTransport,
        I: Iterator<Item = &'i [u8]> + Clone,
        G: FnOnce() -> Result<Vec<AuthorityGrant>, Refusal>,
    {
        if self.retained.body_id != self.bootstrap.body_id
            || self.retained.host_id != self.actual.host_id
            || self.key.verifying_key()
                != SigningKey::from_bytes(self.bootstrap.admission.secret()).verifying_key()
            || !self.bootstrap.matches_native(
                self.actual.host_id.as_str(),
                self.image_id,
                self.actual.profile.as_str(),
            )
        {
            return Err(Refusal::Scope);
        }
        // Hash immutable images once, as on the original MCU path. No decoded
        // offer catalog, duplicate image buffer or new resource pool is made.
        let mut total = 0usize;
        let mut hashes = Vec::new();
        for bytes in images.clone() {
            if hashes.len() >= conduit_body::MAX_MEMBER_RETURN_IMAGES || bytes.is_empty() {
                return Err(Refusal::Bounds);
            }
            total = total.checked_add(bytes.len()).ok_or(Refusal::Bounds)?;
            if total > 24 * 1024 {
                return Err(Refusal::Bounds);
            }
            hashes.push(Sha256::digest(bytes).into());
        }
        let grants = authority_grants()?;
        let projection = BorrowedHostAdvertisementImageProjection {
            authority_grants: &grants,
            context: self.actual,
            capability_images: &hashes,
        };
        projection.validate().map_err(|_| Refusal::Scope)?;
        let signature = self
            .key
            .sign(
                &conduit_body::member_return_image_transcript_into(
                    self.retained,
                    projection,
                    input,
                )
                .map_err(|_| Refusal::Encoding)?,
            )
            .to_bytes();
        let presentation = Presentation {
            kind: "Advertise",
            schema: MEMBER_RETURN_IMAGES_SCHEMA,
            credential: self.retained,
            projection,
            signature: &signature,
        };
        // Reuse the original caller-owned command arena for the signed
        // presentation; no second document or decoded offer catalog is retained.
        let n = serde_json_core::to_slice(&presentation, input).map_err(|_| Refusal::Encoding)?;
        let document = &input[..n];
        let digest = Sha256::digest(document).into();
        let mut offset = 0;
        while offset < document.len() {
            let (n, next) =
                conduit_wire::encode_bounded_document_chunk(output, document, offset, &digest)
                    .map_err(|_| Refusal::Encoding)?;
            link.send(&output[..n])
                .await
                .map_err(|_| Refusal::Transport)?;
            offset = next;
        }
        for (bytes, digest) in images.zip(projection.capability_images) {
            let mut offset = 0;
            while offset < bytes.len() {
                let (n, next) =
                    conduit_wire::encode_bounded_document_chunk(output, bytes, offset, digest)
                        .map_err(|_| Refusal::Encoding)?;
                link.send(&output[..n])
                    .await
                    .map_err(|_| Refusal::Transport)?;
                offset = next;
            }
        }
        let bytes = link.receive(input).await.map_err(|_| Refusal::Transport)?;
        let (challenge, used) =
            serde_json_core::from_slice::<BorrowedPartReturnChallenge<'_>>(bytes)
                .map_err(|_| Refusal::Encoding)?;
        // Owner clock and proposal expiry remain the admitting owner's checks.
        // Sign exactly its expiry-bearing transcript; no cross-Boot clock guess.
        if used != bytes.len()
            || !challenge.matches_member(
                self.retained.body_id.as_str(),
                self.retained.part_id.as_str(),
                self.actual.host_id.as_str(),
                self.actual.boot_id.as_str(),
                self.actual.offer_generation.0,
            )
        {
            return Err(Refusal::Challenge);
        }
        let signature = self.key.sign(&challenge.signing_transcript()).to_bytes();
        let proof = BorrowedPartReturnProof {
            admission_id: challenge.admission_id,
            body_id: challenge.body_id,
            part_id: challenge.part_id,
            host_id: challenge.host_id,
            boot_id: challenge.boot_id,
            nonce: challenge.nonce,
            signature: &signature,
        };
        let n = serde_json_core::to_slice(&proof, output).map_err(|_| Refusal::Encoding)?;
        link.send(&output[..n])
            .await
            .map_err(|_| Refusal::Transport)?;
        let bytes = link.receive(output).await.map_err(|_| Refusal::Transport)?;
        let (metadata, used) =
            serde_json_core::from_slice::<BorrowedAdmissionReceiptAttestation<'_>>(bytes)
                .map_err(|_| Refusal::Encoding)?;
        if used != bytes.len() {
            return Err(Refusal::Encoding);
        }
        let mut receipt = conduit_wire::BorrowedBoundedDocumentAssembly::new(input, 32);
        link.receive_document(&mut receipt)
            .await
            .map_err(|_| Refusal::Transport)?;
        let raw = receipt.completed().ok_or(Refusal::Receipt)?.0;
        metadata
            .verify(
                raw,
                &self.bootstrap.owner,
                &self.key.verifying_key().to_bytes(),
            )
            .map_err(|_| Refusal::Attestation)?;
        let mut decoder = serde_json_core::de::Deserializer::new(raw, None);
        let credential = MemberReturnImageAcknowledgementSeed {
            retained: self.retained,
            actual: projection,
        }
        .deserialize(&mut decoder)
        .map_err(|_| Refusal::Receipt)?;
        let used = decoder.end().map_err(|_| Refusal::Encoding)?;
        if used != raw.len() {
            return Err(Refusal::Encoding);
        }
        let accepted = AcceptedMembership {
            credential,
            receipt_digest: Sha256::digest(raw).into(),
        };
        input.fill(0);
        output.fill(0);
        Ok(accepted)
    }
}
