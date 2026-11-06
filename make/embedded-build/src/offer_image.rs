//! Fabrication-only lossless metadata; publication and authority stay with Root.
use conduit_core::CapabilityOffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityOfferImageError {
    Contract,
    Encoding,
    Bounds,
}

/// Encode an ordinary definition once for flash-backed publication. The image
/// contains no evidence of initialization, current membership or local custody.
pub fn generate_capability_offer_image(
    offer: &CapabilityOffer,
    maximum_image_bytes: usize,
) -> Result<Vec<u8>, CapabilityOfferImageError> {
    use CapabilityOfferImageError as E;
    offer
        .validate_constructed_semantic_contract()
        .map_err(|_| E::Contract)?;
    let json = serde_json::to_vec(offer).map_err(|_| E::Encoding)?;
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&json, 6);
    let document = conduit_wire::DeflatedDocument::from_parts(&compressed, json.len())
        .map_err(|_| E::Encoding)?;
    let length = conduit_wire::DEFLATED_DOCUMENT_HEADER_BYTES
        .checked_add(compressed.len())
        .ok_or(E::Bounds)?;
    if length > maximum_image_bytes {
        return Err(E::Bounds);
    }
    let mut image = vec![0; conduit_wire::DEFLATED_DOCUMENT_HEADER_BYTES];
    document.write_header(&mut image).map_err(|_| E::Encoding)?;
    image.extend_from_slice(&compressed);
    Ok(image)
}
