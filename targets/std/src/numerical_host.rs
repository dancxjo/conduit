//! Transactional source-specific offer preparation before the ordinary planner.
use crate::{installed_std, kernel_preparation, normalize_capability_offers, StdHost};
use conduit_core::{
    ConfigurationValue, ExactNumericWork, NumericalRealizationRequest, OfferGeneration,
};
use conduit_plot::ExpandedCanonicalPlot;
impl StdHost {
    /// Initialize the finite numerical strategies supported by this std Host.
    /// This changes its advertised generation; plans must use the resulting
    /// advertisement. Active resource reservations fence reconfiguration.
    pub fn prepare_numerical_realizations(
        &mut self,
        plot: &ExpandedCanonicalPlot,
    ) -> Result<(), String> {
        plot.validate_expansion()
            .map_err(|why| format!("numeric source identity: {why}"))?;
        let gears = plot
            .gears
            .iter()
            .filter(|gear| {
                gear.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND
            })
            .take(33)
            .collect::<Vec<_>>();
        if gears.is_empty() {
            return Ok(());
        }
        if gears.len() > 32 {
            return Err("std numerical source preparation exceeds 32 instances".into());
        }
        if !self.kernel_resources.is_idle() {
            return Err("numeric offers cannot change active reservations".into());
        }
        let mut advertisement = self.advertisement.clone();
        // Replace this Host's previous source batch while preserving other offers.
        advertisement
            .capabilities
            .retain(|offer| !installed_std::owns_numerical_offer(offer));
        advertisement
            .capabilities
            .push(conduit_std_offers::numerical_presentation_std_offer());
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
