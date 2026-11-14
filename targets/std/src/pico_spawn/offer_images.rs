//! Preparing-Host image resolution; no membership or execution is implied.
use super::{PicoSpawnSocket, PicoSpawnTransportError as Error};
use conduit_body::HostAdvertisementImageProjection;
use conduit_core::HostAdvertisement;
use conduit_wire::{CapabilityOfferImage, CapabilityOfferImageLimits};
use std::time::Duration;

pub(super) const MAX_IMAGE_BYTES: usize = 32 * 1024;
pub(super) const MAX_TOTAL_IMAGE_BYTES: usize = 24 * 1024;
const MAX_EXPANDED_OFFER_BYTES: usize = 256 * 1024;
const MAX_TOTAL_EXPANDED_BYTES: usize = 512 * 1024;
const MAX_IMAGE_CHUNKS: usize = 16;

/// Resolve exact bytes independently at the owner before comparing them with
/// current admitted Host offers. The limits belong to this hosted receiver,
/// never to claims supplied by a device or local control caller.
pub fn resolve_member_offer_images(
    projection: &HostAdvertisementImageProjection,
    images: &[Vec<u8>],
) -> Result<HostAdvertisement, Error> {
    projection
        .validate()
        .map_err(|_| Error::WrongAdvertisement)?;
    if images.len() != projection.capability_images.len() {
        return Err(Error::WrongAdvertisement);
    }
    let images: Vec<_> = projection
        .capability_images
        .iter()
        .zip(images)
        .map(|(digest, bytes)| CapabilityOfferImage {
            sha256: *digest,
            bytes,
        })
        .collect();
    let mut scratch = vec![0; MAX_EXPANDED_OFFER_BYTES];
    let offers = conduit_wire::resolve_capability_offer_images(
        &images,
        &mut scratch,
        CapabilityOfferImageLimits {
            maximum_offers: conduit_body::MAX_MEMBER_RETURN_IMAGES,
            maximum_image_bytes: MAX_IMAGE_BYTES,
            maximum_expanded_offer_bytes: MAX_EXPANDED_OFFER_BYTES,
            maximum_total_image_bytes: MAX_TOTAL_IMAGE_BYTES,
            maximum_total_expanded_bytes: MAX_TOTAL_EXPANDED_BYTES,
        },
    )
    .map_err(|_| Error::WrongAdvertisement)?;
    let mut advertisement = projection.context.clone();
    advertisement.capabilities = offers;
    Ok(advertisement)
}
impl PicoSpawnSocket {
    pub(super) fn receive_offer_images(
        &mut self,
        projection: &HostAdvertisementImageProjection,
        timeout: Duration,
    ) -> Result<Vec<Vec<u8>>, Error> {
        projection
            .validate()
            .map_err(|_| Error::WrongAdvertisement)?;
        let mut remaining = MAX_TOTAL_IMAGE_BYTES;
        let mut images = Vec::with_capacity(projection.capability_images.len());
        let mut frame = [0; conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES];
        for expected in &projection.capability_images {
            let mut assembly = conduit_wire::BoundedDocumentAssembly::new(
                remaining.min(MAX_IMAGE_BYTES),
                MAX_IMAGE_CHUNKS,
            );
            loop {
                let raw = self.line.receive_raw_stream_frame(&mut frame, timeout)?;
                if assembly.push(raw).map_err(|_| Error::Malformed)? {
                    break;
                }
            }
            let (bytes, digest) = assembly.completed().ok_or(Error::Malformed)?;
            if digest != expected {
                return Err(Error::WrongAdvertisement);
            }
            remaining = remaining.checked_sub(bytes.len()).ok_or(Error::Oversized)?;
            images.push(bytes.to_vec());
        }
        Ok(images)
    }
}
