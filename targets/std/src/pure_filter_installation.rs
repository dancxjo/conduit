//! Install only the exact pure filter derived from a checked source program.
use crate::{kernel_preparation::KernelResourceLedger, StdHost};
impl StdHost {
    pub fn install_pure_filter(
        &mut self,
        program: &conduit_plot::PortableExpressionProgram,
        temporal: conduit_core::PortTemporal,
    ) -> Result<(), String> {
        let offer = conduit_std_offers::pure_filter_std_offer(program, temporal)
            .map_err(|error| format!("pure filter contract: {error:?}"))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.push(offer);
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        Ok(())
    }
}
