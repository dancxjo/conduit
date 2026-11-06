//! Canonical retained Part return over the already selected bounded USB carrier.
use super::*;
use conduit_body::{
    BorrowedPartReturnChallenge, BorrowedPartReturnProof, HostAdvertisementImageProjection,
    MemberReturnImageAcknowledgement, MEMBER_RETURN_IMAGES_SCHEMA,
};
use serde::Serialize;
#[derive(Serialize)]
struct Presentation<'a> {
    kind: &'static str,
    schema: &'static str,
    credential: &'a conduit_body::MembershipCredential,
    projection: &'a HostAdvertisementImageProjection,
    signature: &'a [u8],
}
impl MembershipState {
    pub(super) async fn serve_return(
        &mut self,
        link: &mut Link,
        input: &mut [u8; LIMIT],
    ) -> Result<(), ()> {
        self.refresh_native_offer()?;
        let retained = self
            .receipt
            .as_ref()
            .or_else(|| self.seed.admission.credential())
            .ok_or(())?;
        let owner = crate::resident::inventory().map_err(|_| ())?;
        let images = owner.offers();
        let images = images.into_iter().flatten().chain(self.native_offer);
        let mut total = 0usize;
        let mut hashes = Vec::new();
        for bytes in images.clone() {
            total = total.checked_add(bytes.len()).ok_or(())?;
            if total > 24 * 1024 {
                return Err(());
            }
            hashes.push(Sha256::digest(bytes).into());
        }
        let projection = HostAdvertisementImageProjection {
            context: self.resources.host().clone(),
            capability_images: hashes,
        };
        projection.validate().map_err(|_| ())?;
        let signature = self
            .key
            .sign(
                &conduit_body::member_return_image_transcript(retained, &projection)
                    .map_err(|_| ())?,
            )
            .to_bytes();
        let mut output = [0; LIMIT];
        let presentation = Presentation {
            kind: "Advertise",
            schema: MEMBER_RETURN_IMAGES_SCHEMA,
            credential: retained,
            projection: &projection,
            signature: &signature,
        };
        let n = serde_json_core::to_slice(&presentation, &mut output).map_err(|_| ())?;
        link.send(&output[..n]).await?;
        for (bytes, digest) in images.zip(&projection.capability_images) {
            let mut offset = 0;
            while offset < bytes.len() {
                let (n, next) =
                    conduit_wire::encode_bounded_document_chunk(&mut output, bytes, offset, digest)
                        .map_err(|_| ())?;
                link.send(&output[..n]).await?;
                offset = next;
            }
        }
        let bytes = link.receive(input).await?;
        let (challenge, used) =
            serde_json_core::from_slice::<BorrowedPartReturnChallenge<'_>>(bytes)
                .map_err(|_| ())?;
        if used != bytes.len()
            || !challenge.matches_member(
                retained.body_id.as_str(),
                retained.part_id.as_str(),
                self.resources.host().host_id.as_str(),
                self.resources.host().boot_id.as_str(),
                self.resources.host().offer_generation.0,
            )
        {
            return Err(());
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
        let n = serde_json_core::to_slice(&proof, &mut output).map_err(|_| ())?;
        link.send(&output[..n]).await?;
        let bytes = link.receive(input).await?;
        let (metadata, used) =
            serde_json_core::from_slice::<BorrowedAdmissionReceiptAttestation<'_>>(bytes)
                .map_err(|_| ())?;
        if used != bytes.len() {
            return Err(());
        }
        let raw = link.receive(&mut output).await?;
        metadata
            .verify(raw, &self.seed.owner, &self.key.verifying_key().to_bytes())
            .map_err(|_| ())?;
        let (ack, used) =
            serde_json_core::from_slice::<MemberReturnImageAcknowledgement>(raw).map_err(|_| ())?;
        if used != raw.len() {
            return Err(());
        }
        ack.validate_for(retained, &projection).map_err(|_| ())?;
        self.receipt = Some(ack.credential);
        retain_receipt_digest(raw);
        input.fill(0);
        output.fill(0);
        Ok(())
    }
}

impl MembershipState {
    fn refresh_native_offer(&mut self) -> Result<(), ()> {
        if !self.resources.admissions().is_empty() || !self.resources.assignments().is_empty() {
            return Err(());
        }
        let Some(available) = crate::i2c_owner::native_available() else {
            return Ok(());
        };
        let available = available && crate::config::I2C_OFFER_IMAGE.is_some();
        if available == self.native_offer.is_some() {
            return Ok(());
        }
        let used = crate::heap::usage().0;
        let remaining = crate::config::HEAP_BYTES.checked_sub(used).ok_or(())?;
        let context =
            crate::native_publication::prepare(self.resources.host(), available, remaining)
                .map_err(|_| ())?;
        // The idle fence above forbids dropping any original reservation/lane.
        // Context preparation is fallible and leaves the old owner unchanged.
        self.resources = conduit_core::ResourceAdmissionOwner::new(context);
        self.native_offer = if available {
            crate::config::I2C_OFFER_IMAGE
        } else {
            None
        };
        Ok(())
    }
}
