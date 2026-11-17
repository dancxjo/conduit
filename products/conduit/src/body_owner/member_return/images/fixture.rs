//! Compact image return reuses the original durable member-return transaction.
//! Image/grant attribution is descriptive; no native possession is inferred.
use super::super::*;
use conduit_body::{
    HostAdvertisementImageProjection, MemberReturnImageAcknowledgement,
    MemberReturnImagePresentation, SignedPortableAdmissionReceipt,
};
use conduit_wire::{CapabilityOfferImage, CapabilityOfferImageLimits, DeflatedDocument};
use sha2::{Digest, Sha256};
const MAX_IMAGES: usize = 32;
const MAX_IMAGE_BYTES: usize = 24 * 1024;
const MAX_EXPANDED_OFFER_BYTES: usize = 256 * 1024;
const MAX_TOTAL_EXPANDED_BYTES: usize = 512 * 1024;
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PendingImageReturn {
    projection: HostAdvertisementImageProjection,
    projection_digest: [u8; 32],
    offer_digest: [u8; 32],
}
fn evidence_digest(value: &impl serde::Serialize) -> Result<[u8; 32], String> {
    Ok(Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?).into())
}
impl PendingImageReturn {
    fn bind(
        projection: HostAdvertisementImageProjection,
        advertisement: &HostAdvertisement,
    ) -> Result<Self, String> {
        Ok(Self {
            projection_digest: evidence_digest(&projection)?,
            offer_digest: evidence_digest(advertisement)?,
            projection,
        })
    }
    fn check(&self, advertisement: &HostAdvertisement) -> Result<(), String> {
        if evidence_digest(&self.projection)? != self.projection_digest {
            return Err("image projection/grants differ from retained signed evidence".into());
        }
        if evidence_digest(advertisement)? != self.offer_digest {
            return Err("full offer differs from retained image return".into());
        }
        Ok(())
    }
}
impl Owner {
    // SOURCE-only candidate: finite extents and fallible allocations below do
    // not yet fund transient decode AND retained metadata on original IMAGE's
    // preparation ledger (#6341). No live/deployment eligibility is implied.
    pub(crate) fn fixture_begin_image_member_return(
        &mut self,
        presentation: MemberReturnImagePresentation,
        images: &[Vec<u8>],
    ) -> Result<PartReturnChallenge, String> {
        // Neither a received challenge nor a public projection may create keys.
        self.member_carrier_peer()?;
        if self.host.is_playing() {
            return Err("body-play-active".into());
        }
        self.admissions
            .as_ref()
            .ok_or("member return has no admission custody")?
            .verify_member_return_image_presentation(
                &self.session.evidence().membership,
                &presentation,
            )
            .map_err(|e| format!("member image presentation: {e:?}"))?;
        require_receipt_extent(&presentation)?;
        let maximum = preflight(&presentation.projection, images)?;
        let advertisement = resolve(&presentation.projection, images, maximum)?;
        let bound = PendingImageReturn::bind(presentation.projection, &advertisement)?;
        let challenge = self.begin_member_return_owned(&presentation.credential, advertisement)?;
        let pending = self
            .pending_member_return
            .as_mut()
            .ok_or("member return pending custody lost")?;
        match &pending.image_projection {
            Some(original) if original != &bound => {
                return Err(
                    "member return image projection differs from original pending request".into(),
                );
            }
            Some(_) => {}
            None if pending.completed.is_some() => {
                return Err(
                    "cannot attach image projection after an ordinary return completed".into(),
                );
            }
            None => pending.image_projection = Some(bound),
        }
        Ok(challenge)
    }
    pub(crate) fn fixture_complete_image_member_return(
        &mut self,
        root: &Path,
        proof: &PartReturnProof,
    ) -> Result<SignedPortableAdmissionReceipt, String> {
        self.member_carrier_peer()?;
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member return pending custody lost")?;
        pending
            .image_projection
            .as_ref()
            .ok_or("member return has no original pending image projection")?
            .check(&pending.advertisement)?;
        if pending.completed.is_none() {
            require_receipt_capacity(root)?;
        }
        // This is the existing authoritative Core/state writer, not an image
        // adapter admission. Failure leaves no signed acceptance to deliver.
        let credential = self.complete_member_return(root, proof)?;
        let pending = self
            .pending_member_return
            .as_ref()
            .ok_or("member returned; pending custody lost")?;
        let bound = pending
            .image_projection
            .as_ref()
            .ok_or("member returned; image projection lost")?;
        bound.check(&pending.advertisement)?;
        let projection = &bound.projection;
        let actual_digest: [u8; 32] =
            Sha256::digest(serde_json::to_vec(&pending.advertisement).map_err(|e| e.to_string())?)
                .into();
        if pending.completed_offer_digest != Some(actual_digest) {
            return Err("member returned; full offer differs from committed return".into());
        }
        self.require_current_native_member(
            &crate::durable_host::owner::MemberPresentationBasis::new(
                credential.clone(),
                pending.advertisement.clone(),
            )?,
        )?;
        let continuity = self
            .admissions
            .as_ref()
            .ok_or("member returned; original manager absent")?
            .current_member_continuity(
                &self.session.evidence().membership,
                &credential.part_id,
                &pending.advertisement,
            )
            .map_err(|e| format!("member returned; current continuity unavailable: {e:?}"))?;
        let receipt = MemberReturnImageAcknowledgement::new(credential, projection.clone());
        let signed = self
            .member_carrier_identity
            .as_ref()
            .ok_or("member returned; original owner signing custody absent")?
            .attest_image_member_return(&receipt, &continuity)
            .map_err(|e| format!("member returned; receipt attestation failed: {e}"))?;
        retain_receipt(root, &signed).map_err(|e| {
            format!("member returned; signed receipt retention failed, delivery unknown: {e}")
        })?;
        Ok(signed)
    }
}
/// Validate all compressed and expanded extents and signed hash order before
/// creating decoder scratch or typed offers. Claimed flash size supplies no RAM.
fn preflight(
    projection: &HostAdvertisementImageProjection,
    images: &[Vec<u8>],
) -> Result<usize, String> {
    projection
        .validate()
        .map_err(|e| format!("invalid return projection: {e:?}"))?;
    let context = &projection.context;
    if context.protocol_version != conduit_core::PROTOCOL_VERSION
        || context.offer_generation.0 == 0
        || [
            context.host_id.as_str(),
            context.boot_id.as_str(),
            context.profile.as_str(),
        ]
        .iter()
        .any(|id| id.is_empty() || id.len() > conduit_body::MAX_LIFECYCLE_ID_BYTES)
    {
        return Err("invalid returning native Host incarnation".into());
    }
    if !conduit_core::bounded_host_advertisement_comparison_supported(&projection.context)
        || images.len() != projection.capability_images.len()
        || images.len() > MAX_IMAGES
    {
        return Err("unsupported or mismatched member-return image context".into());
    }
    let (mut compressed, mut expanded, mut maximum) = (0usize, 0usize, 0usize);
    for (bytes, expected) in images.iter().zip(&projection.capability_images) {
        compressed = compressed
            .checked_add(bytes.len())
            .ok_or("member image extent overflow")?;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES || compressed > MAX_IMAGE_BYTES {
            return Err("member compressed image bound exceeded".into());
        }
        if Sha256::digest(bytes).as_slice() != expected {
            return Err("member image digest mismatch".into());
        }
        let document =
            DeflatedDocument::parse_image(bytes).map_err(|_| "invalid member offer image")?;
        let length = document.expanded_bytes();
        expanded = expanded
            .checked_add(length)
            .ok_or("member expanded extent overflow")?;
        if length > MAX_EXPANDED_OFFER_BYTES || expanded > MAX_TOTAL_EXPANDED_BYTES {
            return Err("member expanded image bound exceeded".into());
        }
        maximum = maximum.max(length);
    }
    Ok(maximum)
}
fn resolve(
    projection: &HostAdvertisementImageProjection,
    images: &[Vec<u8>],
    maximum: usize,
) -> Result<HostAdvertisement, String> {
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(maximum)
        .map_err(|_| "member image scratch allocation refused")?;
    scratch.resize(maximum, 0);
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve_exact(images.len())
        .map_err(|_| "member image view allocation refused")?;
    for (bytes, digest) in images.iter().zip(&projection.capability_images) {
        borrowed.push(CapabilityOfferImage {
            sha256: *digest,
            bytes,
        });
    }
    // Allocation and decoded typed ownership are preparer costs, independently
    // of the owner key lease; these byte ceilings do not prove heap admission.
    let offers = conduit_wire::resolve_capability_offer_images(
        &borrowed,
        &mut scratch,
        CapabilityOfferImageLimits {
            maximum_offers: MAX_IMAGES,
            maximum_image_bytes: MAX_IMAGE_BYTES,
            maximum_total_image_bytes: MAX_IMAGE_BYTES,
            maximum_expanded_offer_bytes: MAX_EXPANDED_OFFER_BYTES,
            maximum_total_expanded_bytes: MAX_TOTAL_EXPANDED_BYTES,
        },
    )
    .map_err(|e| format!("member offer image resolution refused: {e:?}"))?;
    let mut advertisement = projection.context.clone();
    advertisement.capabilities = offers;
    Ok(advertisement)
}
// Reuse existing admission-history and framed-receipt ceilings. This finite
// evidence store confers no membership or execution authority.
const MAX_RECEIPTS: usize = conduit_body::MAX_ADMISSION_RECEIPTS;
const MAX_STORED_RECEIPT_BYTES: u64 = (2 * conduit_body::MAX_ATTESTED_ADMISSION_RECEIPT_BYTES
    + conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES) as u64;
fn receipt_path(root: &Path, encoded: &[u8]) -> Result<std::path::PathBuf, String> {
    let hash = crate::durable_host::digest(encoded);
    let hash = hash
        .strip_prefix("sha256:")
        .ok_or("invalid receipt content digest")?;
    Ok(root
        .join("body/member-return-receipts")
        .join(format!("{hash}.json")))
}
fn require_receipt_capacity(root: &Path) -> Result<(), String> {
    let directory = root.join("body/member-return-receipts");
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "member receipt retention unavailable before commit: {error}"
            ))
        }
    };
    let mut count = 0;
    for entry in entries.take(MAX_RECEIPTS) {
        entry.map_err(|e| e.to_string())?;
        count += 1;
    }
    if count >= MAX_RECEIPTS {
        return Err("member receipt retention capacity exhausted".into());
    }
    Ok(())
}
fn retain_receipt(root: &Path, signed: &SignedPortableAdmissionReceipt) -> Result<(), String> {
    let encoded = serde_json::to_vec(signed).map_err(|e| e.to_string())?;
    if encoded.len() as u64 > MAX_STORED_RECEIPT_BYTES || signed.receipt_json.len() > 16 * 1024 {
        return Err("signed member receipt exceeds finite retention bound".into());
    }
    let path = receipt_path(root, &encoded)?;
    let directory = path.parent().ok_or("receipt path has no parent")?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    crate::durable_host::restrict_directory(directory)?;
    if path.exists() {
        if crate::durable_host::bounded_read(&path, MAX_STORED_RECEIPT_BYTES)? != encoded {
            return Err("immutable member receipt content differs".into());
        }
        return Ok(());
    }
    if std::fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .take(MAX_RECEIPTS + 1)
        .count()
        >= MAX_RECEIPTS
    {
        return Err("member receipt retention capacity exhausted".into());
    }
    crate::durable_host::write_bytes_atomic(&path, &encoded)
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Bound every possible returned credential without constructing one or guessing
/// Core's future identity spelling. Exact relationship IDs/Boot are already
/// known; a maximum-length, maximally escaped credential ID and u64 timestamp
/// upper-bound the two values that completion will choose. This may refuse a
/// near-limit projection conservatively; it never enlarges carriage capacity.
fn require_receipt_extent(presentation: &MemberReturnImagePresentation) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Credential<'a> {
        credential_id: &'a str,
        body_id: &'a conduit_body::BodyId,
        part_id: &'a conduit_body::PartId,
        host_id: &'a conduit_core::HostId,
        boot_id: &'a conduit_core::BootId,
        issued_at_millis: u64,
    }
    #[derive(serde::Serialize)]
    struct Acknowledgement<'a> {
        kind: &'static str,
        schema: &'static str,
        credential: Credential<'a>,
        projection: &'a HostAdvertisementImageProjection,
    }
    struct ByteLimit(usize);
    impl std::io::Write for ByteLimit {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let maximum_id = [0u8; conduit_body::MAX_LIFECYCLE_ID_BYTES];
    let maximum_id = core::str::from_utf8(&maximum_id).expect("zero bytes are UTF-8");
    let credential = &presentation.credential;
    serde_json::to_writer(
        ByteLimit(16 * 1024),
        &Acknowledgement {
            kind: "Admitted",
            schema: conduit_body::MEMBER_RETURN_IMAGES_SCHEMA,
            credential: Credential {
                credential_id: maximum_id,
                body_id: &credential.body_id,
                part_id: &credential.part_id,
                host_id: &credential.host_id,
                boot_id: &presentation.projection.context.boot_id,
                issued_at_millis: u64::MAX,
            },
            projection: &presentation.projection,
        },
    )
    .map_err(|_| "member return exceeds receipt carriage bound".to_string())
}
