//! Preparation-side resolution of immutable offer images, never admission.
use crate::{DeflatedDocument, DeflatedDocumentLimits};
use alloc::vec::Vec;
use conduit_core::CapabilityOffer;
use sha2::{Digest, Sha256};

/// The expected digest comes from the caller's exact publication context. A
/// digest checks bytes, not who published them or whether they are current.
pub struct CapabilityOfferImage<'a> {
    pub sha256: [u8; 32],
    pub bytes: &'a [u8],
}

/// Independent preparing-Host limits. Neither image headers nor firmware's
/// flash capacity enlarge these budgets. Retained typed records and decoder
/// working state also require preparation workspace, beyond `scratch` itself.
#[derive(Clone, Copy)]
pub struct CapabilityOfferImageLimits {
    pub maximum_offers: usize,
    pub maximum_image_bytes: usize,
    pub maximum_expanded_offer_bytes: usize,
    pub maximum_total_image_bytes: usize,
    pub maximum_total_expanded_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityOfferImageRefusal {
    Bounds,
    Digest,
    Document,
    Encoding,
    Contract,
    DuplicateCapability,
}

/// Resolve the complete ordered list, or return no records. Preflight every
/// extent before hashing, decoding or allocating retained offers. Each image
/// reuses the caller's scratch; decoded JSON is not retained alongside the
/// typed contracts. Scratch may change on refusal and must remain unpublished.
///
/// This is an opt-in preparation decoder, not an MCU execution requirement.
/// The caller still owns Host/Boot/generation binding, authenticated publication,
/// membership, Base associations, resource admission and exact Plan checks.
pub fn resolve_capability_offer_images(
    images: &[CapabilityOfferImage<'_>],
    scratch: &mut [u8],
    limits: CapabilityOfferImageLimits,
) -> Result<Vec<CapabilityOffer>, CapabilityOfferImageRefusal> {
    use CapabilityOfferImageRefusal as R;
    if images.len() > limits.maximum_offers {
        return Err(R::Bounds);
    }
    let (mut image_bytes, mut expanded_bytes) = (0usize, 0usize);
    for image in images {
        if image.bytes.len() > limits.maximum_image_bytes {
            return Err(R::Bounds);
        }
        let document = DeflatedDocument::parse_image(image.bytes).map_err(|_| R::Document)?;
        if document.expanded_bytes() > limits.maximum_expanded_offer_bytes
            || document.expanded_bytes() > scratch.len()
        {
            return Err(R::Bounds);
        }
        image_bytes = image_bytes
            .checked_add(image.bytes.len())
            .ok_or(R::Bounds)?;
        expanded_bytes = expanded_bytes
            .checked_add(document.expanded_bytes())
            .ok_or(R::Bounds)?;
        if image_bytes > limits.maximum_total_image_bytes
            || expanded_bytes > limits.maximum_total_expanded_bytes
        {
            return Err(R::Bounds);
        }
    }
    for image in images {
        let actual: [u8; 32] = Sha256::digest(image.bytes).into();
        if actual != image.sha256 {
            return Err(R::Digest);
        }
    }
    let mut offers: Vec<CapabilityOffer> = Vec::with_capacity(images.len());
    for image in images {
        let document = DeflatedDocument::parse_image(image.bytes).map_err(|_| R::Document)?;
        let json = document
            .decode_into(
                scratch,
                DeflatedDocumentLimits {
                    maximum_expanded_bytes: limits.maximum_expanded_offer_bytes,
                    maximum_compressed_bytes: limits.maximum_image_bytes,
                },
            )
            .map_err(|_| R::Document)?;
        let offer: CapabilityOffer = serde_json::from_slice(json).map_err(|_| R::Encoding)?;
        offer
            .validate_constructed_semantic_contract()
            .map_err(|_| R::Contract)?;
        offer.validate_state_retention().map_err(|_| R::Contract)?;
        if offers
            .iter()
            .any(|prior| prior.capability_id == offer.capability_id)
        {
            return Err(R::DuplicateCapability);
        }
        offers.push(offer);
    }
    Ok(offers)
}
