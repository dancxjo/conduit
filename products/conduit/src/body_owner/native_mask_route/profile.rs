//! Ephemeral offer facts stay separate from durable membership and authority.
use super::*;
use conduit_conduitos_mask_offer::NativeMaskStorageProfile;

pub(crate) struct InvitedMaskOffer {
    pub(super) digest: String,
    boot: conduit_core::BootId,
    generation: conduit_core::OfferGeneration,
    transfer: NativeMaskStorageProfile,
}
impl InvitedMaskOffer {
    pub(crate) fn observed(host: &HostAdvertisement, digest: String) -> Self {
        Self {
            digest,
            boot: host.boot_id.clone(),
            generation: host.offer_generation,
            transfer: NativeMaskStorageProfile::owner_document_profile(host),
        }
    }
}
impl Owner {
    pub(crate) fn native_face_response_bytes(&self, request: &OwnerFaceSnapshotRequest) -> usize {
        let admitted = self.admissions.as_ref().is_some_and(|manager| {
            manager.receipts.iter().any(|receipt| {
                let c = &receipt.credential;
                request.has_exact_basis()
                    && request.credential_id == c.credential_id.as_str()
                    && request.body_id == c.body_id
                    && request.part_id == c.part_id
                    && request.host_id == c.host_id
                    && request.boot_id == c.boot_id
            })
        });
        if admitted && self.session.evidence().body_id == request.body_id {
            if let Some(offer) = self.invited_mask_offers.get(&request.host_id) {
                let current = offer.boot == request.boot_id
                    && self.session.evidence().membership.parts.iter().any(|part| {
                        part.part_id == request.part_id
                            && part.current.as_ref().is_some_and(|host| {
                                host.host_id == request.host_id
                                    && host.boot_id == offer.boot
                                    && host.offer_generation == offer.generation
                            })
                    });
                if current {
                    return offer.transfer.owner_response_bytes();
                }
            }
        }
        NativeMaskStorageProfile::Standard.owner_response_bytes()
    }
}
