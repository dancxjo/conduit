//! Borrow offer bytes only while their initialized software owner is retained.
use super::*;

#[derive(Clone, Copy)]
pub struct ResidentSoftwareOffer<'a> {
    pub(super) node: u16,
    pub(super) json: &'a [u8],
    #[cfg_attr(
        not(feature = "local-planning"),
        allow(dead_code, reason = "linked decoder is used only by local planning")
    )]
    decode: Option<fn() -> Result<c::CapabilityOffer, ()>>,
    // Linked firmware borrows these descriptors from immutable Make material.
    // Dispatch must not retain a JSON fallback in the linked-only path.
    #[cfg(feature = "local-planning")]
    driver: fn(&ResidentSoftwareOffer<'_>) -> Result<c::CapabilityOffer, NativePreparationRefusal>,
}
impl<'a> ResidentSoftwareOffer<'a> {
    /// Retain compiler-produced metadata, never incoming assignment descriptors.
    /// # Safety
    /// Bytes must be the exact compiler-produced CapabilityOffer JSON for this node's
    /// software artifact in the same independently pinned Make recipe. They must
    /// contain no Host/Boot, native provider, resource or authority facts. This
    /// constructor establishes no readiness and initializes no implementation.
    pub const unsafe fn from_make(node: u16, json: &'a [u8]) -> Self {
        Self {
            node,
            json,
            decode: None,
            #[cfg(feature = "local-planning")]
            driver: decode_json,
        }
    }
    /// Link a Make-generated decoder whose exact complete result equals `json`.
    /// # Safety
    /// The existing `from_make` contract applies to both representations. Make
    /// must check complete equality, including every default and semantic law.
    /// Static descriptions establish no initialized software or native readiness.
    pub const unsafe fn from_linked_make(
        node: u16,
        json: &'a [u8],
        decode: fn() -> Result<c::CapabilityOffer, ()>,
    ) -> Self {
        Self {
            node,
            json,
            decode: Some(decode),
            #[cfg(feature = "local-planning")]
            driver: decode_linked,
        }
    }
    #[cfg(feature = "local-planning")]
    pub(super) fn decode(&self) -> Result<c::CapabilityOffer, NativePreparationRefusal> {
        (self.driver)(self)
    }
}
#[cfg(feature = "local-planning")]
fn decode_json(
    offer: &ResidentSoftwareOffer<'_>,
) -> Result<c::CapabilityOffer, NativePreparationRefusal> {
    serde_json::from_slice(offer.json).map_err(|_| NativePreparationRefusal::Configuration)
}
#[cfg(feature = "local-planning")]
fn decode_linked(
    offer: &ResidentSoftwareOffer<'_>,
) -> Result<c::CapabilityOffer, NativePreparationRefusal> {
    offer
        .decode
        .ok_or(NativePreparationRefusal::Configuration)?()
    .map_err(|_| NativePreparationRefusal::Configuration)
}
/// The borrow prevents transferring the corresponding owner while publishing.
/// Outer Host/Boot/offer-generation provenance remains Root's responsibility.
pub struct ReadyResidentOffers<'a, V> {
    _owner: &'a PreparedResidentSoftware<V>,
    offers: &'a [ResidentSoftwareOffer<'a>],
    array_bytes: usize,
}
impl<'a, V> ReadyResidentOffers<'a, V> {
    /// Exact JSON array extent, including brackets and separating commas.
    pub fn array_bytes(&self) -> usize {
        self.array_bytes
    }
    #[cfg(feature = "local-planning")]
    pub fn decode_at(&self, index: usize) -> Result<c::CapabilityOffer, NativePreparationRefusal> {
        self.offers
            .get(index)
            .ok_or(NativePreparationRefusal::Configuration)?
            .decode()
    }
    #[cfg(feature = "local-planning")]
    pub fn decoded(
        &self,
    ) -> impl ExactSizeIterator<Item = Result<c::CapabilityOffer, NativePreparationRefusal>> + '_
    {
        self.offers.iter().map(ResidentSoftwareOffer::decode)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &'a [u8]> + '_ {
        self.offers.iter().map(|offer| offer.json)
    }
}
impl<V: ValueStorage> PreparedResidentSoftware<V> {
    /// Preflight all metadata before lending any bytes. Static descriptors alone
    /// cannot call this method; the complete software set must be initialized.
    pub fn offers<'a>(
        &'a self,
        resident: &ResidentAppliance<'_>,
        offers: &'a [ResidentSoftwareOffer<'a>],
    ) -> Result<ReadyResidentOffers<'a, V>, NativePreparationRefusal> {
        self.source_offers(&resident.software_spec(), offers)
    }
    /// Lend availability from Source-only construction, before graph admission.
    pub fn source_offers<'a>(
        &'a self,
        resident: &ResidentSoftwareSpec<'_>,
        offers: &'a [ResidentSoftwareOffer<'a>],
    ) -> Result<ReadyResidentOffers<'a, V>, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let expected = resident_software::Construction::prepare(
            resident,
            self.construction.count,
            self.construction.period_ms,
        )?;
        if self.construction != expected || offers.len() != NODES - 2 {
            return Err(E::Configuration);
        }
        let mut seen = [false; NODES];
        let mut array_bytes = 2 + offers.len().saturating_sub(1);
        for offer in offers {
            let node = usize::from(offer.node);
            if node >= NODES
                || offer.node == self.construction.roles.pulse.0
                || offer.node == self.construction.roles.tick.0
                || seen[node]
                || offer.json.is_empty()
            {
                return Err(E::Configuration);
            }
            array_bytes = array_bytes
                .checked_add(offer.json.len())
                .ok_or(E::Configuration)?;
            seen[node] = true;
        }
        Ok(ReadyResidentOffers {
            _owner: self,
            offers,
            array_bytes,
        })
    }
}
