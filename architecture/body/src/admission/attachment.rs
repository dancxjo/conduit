//! Membership attachment identity, distinct from complete equipment descriptions.
use super::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, MembershipCredential,
    MAX_ADMISSION_RECEIPTS,
};
use crate::identity::bind_identity;
use crate::{
    AdmissionId, AuthenticatedHostObservation, BodyMembership, MembershipCredentialId,
    MembershipProofId, PartId,
};
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};

/// Borrowed identity fields consumed by invitation membership authentication.
/// This is not an authenticated observation, validated equipment description,
/// current-offer eligibility, resource reservation, or permission to execute.
/// Full equipment/provenance validation remains with the separate offer path.
#[derive(Clone, Copy, Debug)]
pub struct SpawnAdmissionHostScope<'a> {
    pub(super) host_id: &'a HostId,
    pub(super) boot_id: &'a BootId,
    pub(super) offer_generation: OfferGeneration,
}
impl<'a> SpawnAdmissionHostScope<'a> {
    /// Supply identity only; ordinary signature, presence and membership checks
    /// still run during completion. Construction grants no authority.
    pub const fn new(
        host_id: &'a HostId,
        boot_id: &'a BootId,
        offer_generation: OfferGeneration,
    ) -> Self {
        Self {
            host_id,
            boot_id,
            offer_generation,
        }
    }
}
impl<'a> From<&'a HostAdvertisement> for SpawnAdmissionHostScope<'a> {
    fn from(advertisement: &'a HostAdvertisement) -> Self {
        Self::new(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        )
    }
}

impl AdmissionManager {
    pub(super) fn attach_for_host(
        &self,
        membership: &mut BodyMembership,
        host: SpawnAdmissionHostScope<'_>,
        now_millis: u64,
        admission_id: &AdmissionId,
        signs: &AdmissionSigns,
    ) -> Result<MembershipCredential, AdmissionRefusal> {
        if membership.body_id != self.body_id {
            return Err(AdmissionRefusal::WrongBody);
        }
        if self.receipts.len() == MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        let part_id = PartId::bind(
            &self.body_id,
            admission_id.as_str(),
            self.receipts.len() as u64,
        )?;
        let credential_id = MembershipCredentialId::bound(bind_identity(
            "membership-credential",
            &[
                self.body_id.as_str(),
                part_id.as_str(),
                host.host_id.as_str(),
                host.boot_id.as_str(),
                admission_id.as_str(),
            ],
            now_millis,
        ));
        let proof_id = MembershipProofId::bind(credential_id.as_str())?;
        let mut next = membership.clone();
        next.admit(
            &self.body_id,
            next.revision,
            part_id.clone(),
            proof_id.clone(),
            signs.part_admitted.clone(),
        )?;
        next.observe_present(
            &self.body_id,
            next.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: (*host.host_id).clone(),
                boot_id: (*host.boot_id).clone(),
                offer_generation: host.offer_generation,
                proof_id,
                sequence: 0,
            },
            signs.host_attached.clone(),
        )?;
        *membership = next;
        Ok(MembershipCredential {
            credential_id,
            body_id: self.body_id.clone(),
            part_id,
            host_id: (*host.host_id).clone(),
            boot_id: (*host.boot_id).clone(),
            issued_at_millis: now_millis,
        })
    }
}
