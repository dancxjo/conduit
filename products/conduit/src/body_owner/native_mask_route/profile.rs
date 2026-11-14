//! Ephemeral offer facts stay separate from durable membership and authority.
use super::*;
use conduit_conduitos_mask_offer::NativeMaskStorageProfile;

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
            if let Some(offer) = self
                .current_host_offers
                .current_hosts(self.session.evidence())
                .find(|offer| {
                    offer.host_id == request.host_id
                        && offer.boot_id == request.boot_id
                        && self.session.evidence().membership.parts.iter().any(|part| {
                            part.part_id == request.part_id
                                && part.state == conduit_body::MembershipState::Admitted
                                && part.current.as_ref().is_some_and(|current| {
                                    current.host_id == offer.host_id
                                        && current.boot_id == offer.boot_id
                                        && current.offer_generation == offer.offer_generation
                                })
                        })
                })
            {
                return NativeMaskStorageProfile::owner_document_profile(offer)
                    .owner_response_bytes();
            }
        }

        NativeMaskStorageProfile::Standard.owner_response_bytes()
    }
}
