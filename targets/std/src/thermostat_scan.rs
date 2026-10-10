//! Installed std Host selection for the ordinary Thermostat scan Plot.
use crate::{composition, flow_activation, StdHost, StdHostComposition, StdHostConfig};
impl StdHost {
    pub fn new_for_thermostat_scan(
        config: StdHostConfig,
        initial: &conduit_thermostat_plot::ThermostatState,
        maximum_items: u16,
    ) -> Result<Self, String> {
        let mut advertisement = composition::build_advertisement(
            config,
            StdHostComposition::reference(),
            None,
            None,
            None,
            false,
        );
        for offer in [
            flow_activation::thermostat_scan_offer(initial, maximum_items)?,
            flow_activation::thermostat_combine_offer(),
        ] {
            if let Some(existing) = advertisement
                .capabilities
                .iter()
                .find(|existing| existing.capability_id == offer.capability_id)
            {
                if existing != &offer {
                    return Err(
                        "std Thermostat scan capability identity conflicts with the selected offer"
                            .into(),
                    );
                }
                continue;
            }
            advertisement.capabilities.push(offer);
        }
        advertisement
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        Self::from_advertisement(advertisement)
    }
}
