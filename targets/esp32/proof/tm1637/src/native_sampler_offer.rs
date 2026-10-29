//! Source-scoped projection of the actual initialized sampler native owners.
//! Checked retained software offers are appended by the existing caller. This
//! projection issues no grant, initializes no provider, and changes no epoch.
use super::{c, NativeRoot};
impl NativeRoot {
    pub(crate) fn sampler_native_advertisement(&self) -> Result<c::HostAdvertisement, ()> {
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
        if let Some(pulse) = &self.pulse {
            // Linked canonical metadata is preparation-only; live calls own their codec.
            let contract = conduit_pulse_capture::PulseReadContract::prepare().map_err(|_| ())?;
            pulse
                .append_to_advertisement(&mut advertisement, &contract)
                .map_err(|_| ())?;
        }
        if let Some(wait) = &self.wait {
            wait.append_to_advertisement(&mut advertisement)
                .map_err(|_| ())?;
        }
        crate::native_offer_order::sort(&mut advertisement);
        Ok(advertisement)
    }
}
