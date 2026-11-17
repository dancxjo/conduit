//! Install only the exact pure filter derived from a checked source program.
use crate::StdHost;
impl StdHost {
    pub fn install_pure_filter(
        &mut self,
        program: &conduit_plot::PortableExpressionProgram,
        temporal: conduit_core::PortTemporal,
    ) -> Result<(), String> {
        self.kernel_resources.require_reference_reconfiguration()?;
        let offer = conduit_std_offers::pure_filter_std_offer(program, temporal)
            .map_err(|error| format!("pure filter contract: {error:?}"))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.push(offer);
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = self.kernel_resources.replacement_for(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        Ok(())
    }
}
