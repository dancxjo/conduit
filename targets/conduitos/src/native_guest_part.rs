//! Guest-local membership observation from one authenticated owner receipt.
//!
//! The owner has already admitted this exact Part. The receipt does not carry
//! the owner's Body biography or workset, so this observation cannot Wake,
//! Plan, or Play the Body. The local membership events are a checked projection
//! of the credential, not a replacement for the owner's retained event log.

use alloc::format;
use conduit_body::{
    AuthenticatedHostObservation, BodyMembership, MembershipCredential, MembershipProofId,
    PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{BootId, HostId, OfferGeneration, SignId};

use crate::spore_join::{OwnerExchange, PendingNativeJoin};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeGuestPart {
    credential: MembershipCredential,
    membership: BodyMembership,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestPartRefusal {
    ReceiptBasis,
    Membership,
}

impl GuestPartRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptBasis => "native-guest-receipt-basis-invalid",
            Self::Membership => "native-guest-membership-invalid",
        }
    }
}

impl NativeGuestPart {
    pub fn credential(&self) -> &MembershipCredential {
        &self.credential
    }

    pub fn is_present(&self) -> bool {
        self.membership
            .parts
            .first()
            .is_some_and(conduit_body::PartMembership::is_present)
    }

    /// Install only the current guest's Part from the owner's verified reply.
    /// The routed exchange has already checked the exact request and pinned
    /// owner transport; this boundary rechecks current product identity.
    pub fn install(
        pending: &PendingNativeJoin,
        receipt: PortableAdmissionReceipt,
        host_id: &HostId,
        boot_id: &BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, GuestPartRefusal> {
        let credential = receipt.credential;
        if pending.owner_exchange != OwnerExchange::ReceiptVerified
            || receipt.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
            || !receipt.membership_admitted
            || receipt.plan_created
            || receipt.play_created
            || receipt.current_offers_available
                != !receipt.host_advertisement.capabilities.is_empty()
            || credential.body_id.as_str() != pending.body_id
            || credential.host_id != *host_id
            || credential.boot_id != *boot_id
            || receipt.host_advertisement.host_id != *host_id
            || receipt.host_advertisement.boot_id != *boot_id
            || receipt.host_advertisement.offer_generation != offer_generation
        {
            return Err(GuestPartRefusal::ReceiptBasis);
        }
        let proof = MembershipProofId::bind(credential.credential_id.as_str())
            .map_err(|_| GuestPartRefusal::Membership)?;
        let body_id = credential.body_id.clone();
        let mut membership =
            BodyMembership::new(body_id.clone()).map_err(|_| GuestPartRefusal::Membership)?;
        membership
            .admit(
                &body_id,
                membership.revision,
                credential.part_id.clone(),
                proof.clone(),
                SignId::from(format!(
                    "conduitos/guest/part-admitted/{}",
                    credential.part_id.as_str()
                )),
            )
            .map_err(|_| GuestPartRefusal::Membership)?;
        membership
            .observe_present(
                &body_id,
                membership.revision,
                &credential.part_id,
                AuthenticatedHostObservation {
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    offer_generation,
                    proof_id: proof,
                    sequence: 0,
                },
                SignId::from(format!(
                    "conduitos/guest/host-present/{}",
                    credential.part_id.as_str()
                )),
            )
            .map_err(|_| GuestPartRefusal::Membership)?;
        membership
            .validate()
            .map_err(|_| GuestPartRefusal::Membership)?;
        Ok(Self {
            credential,
            membership,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};

    fn pending() -> PendingNativeJoin {
        PendingNativeJoin {
            spore_id: "spore/one".into(),
            body_id: "body/owner".into(),
            invitation_id: "invitation/one".into(),
            rendezvous: None,
            route_certificates: vec![],
            owner_exchange: OwnerExchange::ReceiptVerified,
        }
    }

    fn receipt() -> PortableAdmissionReceipt {
        let host = HostId::from("host/guest");
        let boot = BootId::from("boot/guest");
        serde_json::from_value(serde_json::json!({
            "schema": SPAWN_ADMISSION_RECEIPT_SCHEMA,
            "credential": {
                "credential_id": "credential/one", "body_id": "body/owner",
                "part_id": "part/owner-issued", "host_id": "host/guest",
                "boot_id": "boot/guest", "issued_at_millis": 42
            },
            "host_advertisement": HostAdvertisement {
                protocol_version: PROTOCOL_VERSION,
                host_id: host,
                boot_id: boot,
                offer_generation: OfferGeneration(1),
                profile: "profile/guest".into(),
                bases: vec![], resources: vec![], capabilities: vec![],
                planner_capabilities: vec![],
            },
            "membership_admitted": true, "current_offers_available": false,
            "plan_created": false, "play_created": false
        }))
        .unwrap()
    }

    #[test]
    fn exact_owner_part_becomes_checked_guest_membership_without_execution() {
        let installed = NativeGuestPart::install(
            &pending(),
            receipt(),
            &HostId::from("host/guest"),
            &BootId::from("boot/guest"),
            OfferGeneration(1),
        )
        .unwrap();
        assert_eq!(installed.credential.part_id.as_str(), "part/owner-issued");
        assert_eq!(installed.membership.body_id.as_str(), "body/owner");
        assert_eq!(installed.membership.parts.len(), 1);
        assert!(installed.membership.parts[0].is_present());
        assert_eq!(installed.membership.revision.0, 2);
    }

    #[test]
    fn mismatched_or_unverified_receipt_never_installs_a_part() {
        let mut wrong = receipt();
        wrong.credential.body_id = serde_json::from_str("\"body/other\"").unwrap();
        assert_eq!(
            NativeGuestPart::install(
                &pending(),
                wrong,
                &HostId::from("host/guest"),
                &BootId::from("boot/guest"),
                OfferGeneration(1),
            ),
            Err(GuestPartRefusal::ReceiptBasis)
        );
        let mut unverified = pending();
        unverified.owner_exchange = OwnerExchange::Refused("owner-refused");
        assert_eq!(
            NativeGuestPart::install(
                &unverified,
                receipt(),
                &HostId::from("host/guest"),
                &BootId::from("boot/guest"),
                OfferGeneration(1),
            ),
            Err(GuestPartRefusal::ReceiptBasis)
        );
        let mut wrong_boot = receipt();
        wrong_boot.credential.boot_id = serde_json::from_str("\"boot/stale\"").unwrap();
        assert_eq!(
            NativeGuestPart::install(
                &pending(),
                wrong_boot,
                &HostId::from("host/guest"),
                &BootId::from("boot/guest"),
                OfferGeneration(1),
            ),
            Err(GuestPartRefusal::ReceiptBasis)
        );
        let mut claims_play = receipt();
        claims_play.play_created = true;
        assert_eq!(
            NativeGuestPart::install(
                &pending(),
                claims_play,
                &HostId::from("host/guest"),
                &BootId::from("boot/guest"),
                OfferGeneration(1),
            ),
            Err(GuestPartRefusal::ReceiptBasis)
        );
        assert_eq!(
            NativeGuestPart::install(
                &pending(),
                receipt(),
                &HostId::from("host/guest"),
                &BootId::from("boot/guest"),
                OfferGeneration(2),
            ),
            Err(GuestPartRefusal::ReceiptBasis)
        );
    }

    #[test]
    fn joined_face_exposes_exact_body_and_part_without_lifecycle_actions() {
        let host = HostId::from("host/guest");
        let boot = BootId::from("boot/guest");
        let part =
            NativeGuestPart::install(&pending(), receipt(), &host, &boot, OfferGeneration(1))
                .unwrap();
        let mut door = crate::front_door::FrontDoor::new(
            host,
            boot,
            OfferGeneration(1),
            "profile/guest",
            "build/guest",
            "image/guest",
            "source/guest".into(),
            "checked/guest".into(),
            0,
            false,
        );
        door.await_join(pending()).unwrap();
        door.install_join(part.clone()).unwrap();
        assert_eq!(
            door.install_join(part),
            Err(crate::front_door::Error::Presentation)
        );
        let face = door.presentation().unwrap();
        face.validate().unwrap();
        assert_eq!(face.basis.body_id.unwrap().as_str(), "body/owner");
        assert!(face.basis.wake_id.is_none());
        assert!(face.basis.active_play_id.is_none());
        assert!(face.actions.is_empty());
        assert!(face.subjects.iter().any(|subject| {
            subject.role == conduit_presentation::PresentationRole::Part
                && subject.identity == "part/part/owner-issued"
        }));
        assert!(!door.joining_pending());
    }
}
