//! Add one encounter offer to the retained idle Host without replacing its state.
use super::*;
use crate::kernel_preparation::KernelResourceLedger;
use conduit_core::OfferGeneration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrecheAttachmentRefusal {
    HostActive,
    InvalidSlot,
    ExistingEncounter,
    PoolCollision,
    GenerationExhausted,
    InvalidLedger(String),
}
impl StdHost {
    /// Install the already-supported Back and one explicitly supplied slot on
    /// this same idle Host. Existing plans must be replanned at the new offer
    /// generation. Body owners must additionally refuse outstanding proposals.
    /// This grants no source, make, filesystem, display or execution authority.
    pub fn attach_creche_encounter(
        &mut self,
        resource: ResourceOffer,
    ) -> Result<(), CrecheAttachmentRefusal> {
        use CrecheAttachmentRefusal as E;
        if !self.kernel_resources.is_idle() {
            return Err(E::HostActive);
        }
        if resource.class_id.as_str() != CRECHE_RESOURCE
            || resource.pool_id.as_str().trim().is_empty()
            || resource.capacity_units != 1
            || resource.compute.is_some()
            || resource.content.is_some()
        {
            return Err(E::InvalidSlot);
        }
        let offer = creche_std_offer();
        if self
            .advertisement
            .capabilities
            .iter()
            .any(|cap| cap.capability_id == offer.capability_id)
            || self
                .advertisement
                .resources
                .iter()
                .any(|slot| slot.class_id.as_str() == CRECHE_RESOURCE)
        {
            return Err(E::ExistingEncounter);
        }
        if self
            .advertisement
            .resources
            .iter()
            .any(|slot| slot.pool_id == resource.pool_id)
        {
            return Err(E::PoolCollision);
        }
        // Never silently repair or replace an inconsistent current ledger.
        self.kernel_resources
            .observe_offers(&self.advertisement, &[], &[])
            .map_err(E::InvalidLedger)?;
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation = OfferGeneration(
            advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or(E::GenerationExhausted)?,
        );
        advertisement.resources.push(resource);
        advertisement.capabilities.push(offer);
        advertisement.resources.sort();
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let ledger = KernelResourceLedger::new(&advertisement).map_err(E::InvalidLedger)?;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        Ok(())
    }
}
