//! Transactional source-specific offer preparation before the ordinary planner.
use crate::{installed_std, kernel_preparation, normalize_capability_offers, StdHost};
use conduit_core::{
    ConfigurationValue, ExactNumericWork, NumericalRealizationRequest, OfferGeneration,
};
use conduit_plot::ExpandedCanonicalPlot;
mod enclosure_encoding;
pub(crate) mod enclosure_step;
mod expressions;
pub(crate) mod ieee_encoding;
pub(crate) mod ieee_native;
pub(crate) mod ieee_step;
pub(crate) mod near_encoding;
pub(crate) mod near_step;
impl StdHost {
    /// Initialize the finite numerical strategies supported by this std Host.
    /// This changes its advertised generation; plans must use the resulting
    /// advertisement. Active resource reservations fence reconfiguration.
    pub fn prepare_numerical_realizations(
        &mut self,
        plot: &ExpandedCanonicalPlot,
    ) -> Result<(), String> {
        let gears = plot
            .gears
            .iter()
            .filter(|gear| {
                gear.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND
            })
            .take(33)
            .collect::<Vec<_>>();
        let ieee_gears = plot
            .gears
            .iter()
            .filter(|gear| {
                matches!(
                    gear.kind_id.as_str(),
                    conduit_semantic_catalog::IEEE_BINARY64_KIND
                        | conduit_semantic_catalog::IEEE_BINARY64_QUIET_KIND
                )
            })
            .take(33)
            .collect::<Vec<_>>();
        let near_gears = plot
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_NEAR_KIND)
            .take(33)
            .collect::<Vec<_>>();
        let enclosure_gears = plot
            .gears
            .iter()
            .filter(|gear| {
                gear.kind_id.as_str() == conduit_semantic_catalog::MATHEMATICAL_ENCLOSE_KIND
            })
            .take(33)
            .collect::<Vec<_>>();
        if gears.is_empty()
            && ieee_gears.is_empty()
            && near_gears.is_empty()
            && enclosure_gears.is_empty()
            && !self.advertisement.capabilities.iter().any(|offer| {
                installed_std::owns_ieee_offer(offer)
                    || installed_std::owns_near_offer(offer)
                    || installed_std::owns_enclosure_offer(offer)
            })
        {
            return Ok(());
        }
        if gears.len() + ieee_gears.len() + near_gears.len() + enclosure_gears.len() > 32 {
            return Err("std numerical source preparation exceeds 32 instances".into());
        }
        plot.validate_expansion()
            .map_err(|why| format!("numeric source identity: {why}"))?;
        if !self.kernel_resources.is_idle() {
            return Err("numeric offers cannot change active reservations".into());
        }
        let mut advertisement = self.advertisement.clone();
        // Replace this Host's previous source batch while preserving other offers.
        advertisement.capabilities.retain(|offer| {
            !installed_std::owns_numerical_offer(offer)
                && !installed_std::owns_ieee_offer(offer)
                && !installed_std::owns_near_offer(offer)
                && !installed_std::owns_enclosure_offer(offer)
        });
        advertisement
            .capabilities
            .push(conduit_std_offers::numerical_face_std_offer());
        for gear in gears {
            let [entry] = gear.configuration.as_slice() else {
                return Err("numeric source requires one exact request".into());
            };
            let ("request", ConfigurationValue::Structured(value)) =
                (entry.key.as_str(), &entry.value)
            else {
                return Err("numeric source requires its typed request".into());
            };
            let request = NumericalRealizationRequest::from_configuration(
                value,
                &mut ExactNumericWork::new(100_000_000),
            )
            .map_err(|why| format!("numeric source request: {why:?}"))?;
            advertisement
                .capabilities
                .extend(installed_std::numerical_offers(&request)?);
        }
        for gear in ieee_gears {
            conduit_semantic_catalog::ieee_binary64_configuration(
                &gear.kind_id,
                &gear.configuration,
            )?;
            let offer = installed_std::ieee_offer(gear.kind_id.as_str())?;
            if !gear.accepts_realization(&offer) {
                return Err("ieee/source-kind-fore-differs@1".into());
            }
            advertisement.capabilities.push(offer);
        }
        for gear in near_gears {
            advertisement
                .capabilities
                .extend(installed_std::near_offers(&gear.configuration)?);
        }
        for gear in enclosure_gears {
            let offer = installed_std::enclosure_offer(&gear.configuration)?;
            if !gear.accepts_realization(&offer) {
                return Err("numeric/enclose-source-kind-fore-differs@1".into());
            }
            advertisement.capabilities.push(offer);
        }
        normalize_capability_offers(&mut advertisement.capabilities)?;
        if advertisement.capabilities == self.advertisement.capabilities {
            return Ok(());
        }
        advertisement.offer_generation = OfferGeneration(
            advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("numeric offer generation exhausted")?,
        );
        let resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        Ok(())
    }
}
