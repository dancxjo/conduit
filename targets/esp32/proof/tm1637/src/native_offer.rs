//! Canonical projection of resources still owned by this actual Root.
use super::{c, NativeRoot};
impl NativeRoot {
    /// Projection is made from the still-retained provider each time. It is an
    /// offer, not authority, membership, a Body birth or an installed native Play.
    pub fn write_offer(&self, output: &mut [u8]) -> Result<usize, ()> {
        let output = output
            .get_mut(..crate::native_admission::MAXIMUM_ADVERTISEMENT_BYTES)
            .ok_or(())?;
        serde_json_core::to_slice(&self.advertisement()?, output).map_err(|_| ())
    }
    pub(crate) fn advertisement(&self) -> Result<c::HostAdvertisement, ()> {
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
        if let Some(pulse) = &self.pulse {
            // Linked canonical metadata is preparation-only; live calls own their codec.
            let contract = conduit_pulse_capture::PulseReadContract::prepare().map_err(|_| ())?;
            pulse
                .append_to_advertisement(&mut advertisement, &contract)
                .map_err(|_| ())?;
        }
        if let Some(source) = &self.credentials {
            source
                .append_to_advertisement(&mut advertisement)
                .map_err(|_| ())?;
        }
        if let Some(wait) = &self.wait {
            wait.append_to_advertisement(&mut advertisement)
                .map_err(|_| ())?;
        }
        self.display.append_to_advertisement(&mut advertisement)?;
        self.play.append_offer(&mut advertisement);
        self.network.append_to_advertisement(&mut advertisement)?;
        // Providers append their own retained facts. Root composes the complete
        // advertisement in the canonical order required by ordinary planning.
        crate::native_offer_order::sort(&mut advertisement);
        Ok(advertisement)
    }
}
