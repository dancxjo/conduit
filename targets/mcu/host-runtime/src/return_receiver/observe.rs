use super::alloc::vec::Vec;
use super::{Basis, Limits, Refusal};
use crate::transport::DocumentTransport;
use conduit_body::{
    HostAdvertisementImageProjection, MemberReturnImagePresentation, MembershipCredential,
};
use conduit_core::HostAdvertisement;
use conduit_wire::{
    resolve_capability_offer_images, BorrowedBoundedDocumentAssembly, CapabilityOfferImage,
    CapabilityOfferImageLimits,
};
use sha2::{Digest, Sha256};
/// Mutable byte backing funded independently by the receiving Host. The image
/// arena is temporary: no compressed image owner survives `observe`.
pub struct ReceiveBuffers<'a> {
    pub presentation: &'a mut [u8],
    pub images: &'a mut [u8],
    pub decode: &'a mut [u8],
}
/// Signed context plus its resolved offers. Private construction prevents a
/// helper caller from substituting a decoded advertisement after authentication.
pub struct Observation {
    pub(super) presentation: MemberReturnImagePresentation,
    advertisement: HostAdvertisement,
}
impl Observation {
    pub fn projection(&self) -> &HostAdvertisementImageProjection {
        &self.presentation.projection
    }
    pub fn credential(&self) -> &MembershipCredential {
        &self.presentation.credential
    }
    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }
}
/// Authenticate the original returning member before reading or resolving any
/// offer image. This prepares descriptive evidence only, not membership/authority.
/// The caller bounds the entire future with its actual local deadline and
/// dispatches any transport control request before this document exchange. The
/// existing DocumentTransport retains its separately funded framing buffers.
pub async fn observe<T: DocumentTransport>(
    link: &mut T,
    basis: Basis<'_>,
    limits: Limits,
    buffers: ReceiveBuffers<'_>,
) -> Result<Observation, Refusal> {
    basis.validate()?;
    if !limits.valid()
        || buffers.presentation.len() < limits.maximum_presentation_bytes
        || buffers.images.len() < limits.maximum_total_image_bytes
        || buffers.decode.len() < limits.maximum_expanded_offer_bytes
    {
        return Err(Refusal::Bounds);
    }
    let presentation = {
        let mut document = BorrowedBoundedDocumentAssembly::new(
            &mut buffers.presentation[..limits.maximum_presentation_bytes],
            limits.maximum_document_chunks,
        );
        link.receive_document(&mut document)
            .await
            .map_err(|_| Refusal::Transport)?;
        let (raw, digest) = document.completed().ok_or(Refusal::Document)?;
        if Sha256::digest(raw).as_slice() != digest {
            return Err(Refusal::Document);
        }
        serde_json::from_slice::<MemberReturnImagePresentation>(raw)
            .map_err(|_| Refusal::Encoding)?
    };
    basis.check(&presentation)?;
    if presentation.projection.capability_images.len() > limits.maximum_images {
        return Err(Refusal::Bounds);
    }
    let mut images = Vec::new();
    images
        .try_reserve_exact(presentation.projection.capability_images.len())
        .map_err(|_| Refusal::Allocation)?;
    let mut remaining = &mut buffers.images[..limits.maximum_total_image_bytes];
    for expected in &presentation.projection.capability_images {
        let maximum = remaining.len().min(limits.maximum_image_bytes);
        if maximum == 0 {
            return Err(Refusal::Bounds);
        }
        let length = {
            let mut document = BorrowedBoundedDocumentAssembly::new(
                &mut remaining[..maximum],
                limits.maximum_document_chunks,
            );
            link.receive_document(&mut document)
                .await
                .map_err(|_| Refusal::Transport)?;
            let (bytes, digest) = document.completed().ok_or(Refusal::Document)?;
            if digest != expected || Sha256::digest(bytes).as_slice() != expected {
                return Err(Refusal::Document);
            }
            bytes.len()
        };
        let (completed, tail) = remaining.split_at_mut(length);
        images.push(CapabilityOfferImage {
            sha256: *expected,
            bytes: completed,
        });
        remaining = tail;
    }
    let offers = resolve_capability_offer_images(
        &images,
        buffers.decode,
        CapabilityOfferImageLimits {
            maximum_offers: limits.maximum_images,
            maximum_image_bytes: limits.maximum_image_bytes,
            maximum_expanded_offer_bytes: limits.maximum_expanded_offer_bytes,
            maximum_total_image_bytes: limits.maximum_total_image_bytes,
            maximum_total_expanded_bytes: limits.maximum_total_expanded_bytes,
        },
    )
    .map_err(Refusal::Images)?;
    let mut advertisement = presentation.projection.context.clone();
    advertisement.capabilities = offers;
    Ok(Observation {
        presentation,
        advertisement,
    })
}
