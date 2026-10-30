//! Source-scoped projection from the retained display owner. Constructing other
//! providers' decoded metadata and then discarding it consumes the same finite
//! Root workspace without contributing an offer to this Plot.
use super::{c, NativeRoot};
impl NativeRoot {
    pub(crate) fn display_native_advertisement(&self) -> Result<c::HostAdvertisement, ()> {
        let mut advertisement = c::HostAdvertisement {
            declared_name: None,
            protocol_version: c::PROTOCOL_VERSION,
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            offer_generation: c::OfferGeneration(self.offer_generation),
            profile: "environment/esp32-native-root@1".into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            capabilities: alloc::vec![],
            planner_capabilities: alloc::vec![],
        };
        self.display.append_to_advertisement(&mut advertisement)?;
        crate::native_offer_order::sort(&mut advertisement);
        Ok(advertisement)
    }
}
