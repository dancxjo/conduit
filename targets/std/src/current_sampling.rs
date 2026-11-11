//! Prepare exact pure sampling offers on the selected std Host.
use crate::{kernel_preparation::KernelResourceLedger, StdHost};
use conduit_core::CheckedValueContract;

impl StdHost {
    /// Install the coalescing Current and finite sampler specializations before planning.
    pub fn install_finite_current_sampling(
        &mut self,
        value: &CheckedValueContract,
        trigger: &CheckedValueContract,
    ) -> Result<(), String> {
        let sample = conduit_std_offers::current_sample_finite_offer(value, trigger)
            .map_err(str::to_owned)?;
        let current = conduit_std_offers::flow_coalesce_latest_std_offer(
            &value.value_kind,
            value.maximum_bytes,
        );
        let finite_current = conduit_std_offers::flow_coalesce_finite_offer(value);
        let mut advertisement = self.advertisement.clone();
        advertisement
            .capabilities
            .extend([current, finite_current, sample]);
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        Ok(())
    }
}
