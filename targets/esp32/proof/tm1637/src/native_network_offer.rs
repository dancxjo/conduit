//! Current Source-scoped offers for network planning. These are projected from
//! the same retained initialized owners as the complete Root advertisement.
//! No unrelated sampler/Mask metadata is reconstructed, and no grant is minted.
use super::{c, NativeRoot};
impl NativeRoot {
    pub(crate) fn network_advertisement(&self) -> Result<c::HostAdvertisement, ()> {
        let mut advertisement = c::HostAdvertisement {
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
        if let Some(source) = &self.credentials {
            source
                .append_to_advertisement(&mut advertisement)
                .map_err(|_| ())?;
        }
        // This is the same ordinary local attachment-Sign implementation, not
        // an effect provider or a replacement grant for Credentials/Join.
        self.play.append_offer(&mut advertisement);
        self.network.append_to_advertisement(&mut advertisement)?;
        advertisement
            .bases
            .sort_by(|a, b| a.base_id.cmp(&b.base_id));
        advertisement
            .resources
            .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        Ok(advertisement)
    }
}
