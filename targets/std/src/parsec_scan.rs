//! Installed std Host selection for the source-owned retained Parsec scan.
use crate::{composition, flow_activation, StdHost, StdHostComposition, StdHostConfig};

impl StdHost {
    /// Publishes the exact checked-source and retained scan offers before planning.
    ///
    /// This selects software execution capabilities; it does not establish an
    /// installed keyboard binding or current input-map authority.
    pub fn new_for_parsec_scan(config: StdHostConfig, maximum_items: u16) -> Result<Self, String> {
        let mut advertisement = composition::build_advertisement(
            config,
            StdHostComposition::reference(),
            None,
            None,
            None,
            false,
        );
        let mut offers = flow_activation::required_parsec_source_offers()?;
        offers.push(flow_activation::parsec_combine_offer()?);
        offers.push(flow_activation::parsec_scan_offer(maximum_items)?);
        for offer in offers {
            if let Some(existing) = advertisement
                .capabilities
                .iter()
                .find(|existing| existing.capability_id == offer.capability_id)
            {
                if existing != &offer {
                    return Err(
                        "std Parsec scan capability identity conflicts with the selected offer"
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
