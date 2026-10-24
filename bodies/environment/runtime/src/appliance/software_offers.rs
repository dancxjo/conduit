//! Borrowed inspection tied to actual retained portable owners and one current
//! Root offer epoch. This creates no provider, permission, Line or Body.
use super::{ReadySoftwareBacks, SoftwareRefusal};
use conduit_core as c;
pub struct ReadySoftwareOffers<'a> {
    owner: &'a ReadySoftwareBacks,
}
impl ReadySoftwareBacks {
    /// Root supplies its actual current scope. A refusal preserves the previous
    /// selection epoch; success invalidates plans against older offer epochs.
    pub fn current_offers(
        &mut self,
        host: &c::HostId,
        boot: &c::BootId,
        generation: c::OfferGeneration,
    ) -> Result<ReadySoftwareOffers<'_>, SoftwareRefusal> {
        if host != &self.host
            || boot != &self.boot
            || generation.0 == 0
            || self
                .generation
                .is_some_and(|previous| generation.0 < previous.0)
        {
            return Err(SoftwareRefusal::Advertisement);
        }
        self.generation = Some(generation);
        Ok(ReadySoftwareOffers { owner: self })
    }
}
impl ReadySoftwareOffers<'_> {
    pub fn host(&self) -> &c::HostId {
        &self.owner.host
    }
    pub fn boot(&self) -> &c::BootId {
        &self.owner.boot
    }
    pub fn generation(&self) -> c::OfferGeneration {
        self.owner.generation.expect("current borrowed offer")
    }
    pub fn source(&self) -> &c::SourceDocumentId {
        &self.owner.source
    }
    pub fn checked(&self) -> &c::CheckedPlotId {
        &self.owner.checked
    }
    pub fn expanded(&self) -> &c::ExpandedPlotId {
        &self.owner.expanded
    }
    pub fn iter(&self) -> impl Iterator<Item = &c::CapabilityOffer> {
        self.owner
            .operations
            .iter()
            .filter(|op| op.prepared.is_some())
            .map(|op| &op.offer)
    }
}
