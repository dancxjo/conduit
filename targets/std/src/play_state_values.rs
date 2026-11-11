//! Preparation-only installation of existing finite Play-lived State Backs.
use crate::{kernel_preparation::KernelResourceLedger, StdHost};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
impl StdHost {
    /// Select a finite typed State realization before planning. This installs
    /// the existing 4096-byte Play profile, never durable residence or authority.
    pub fn install_play_state_value(
        &mut self,
        ty: &StructuredInfoType,
        initial: &StructuredInfoValue,
    ) -> Result<(), String> {
        if initial
            .canonical_bytes()
            .map_err(|error| format!("State initial value: {error:?}"))?
            .len()
            > conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize
        {
            return Err("State initial value exceeds the existing Play profile".into());
        }
        let offer = conduit_std_offers::state_value_std_offer("Value", ty, initial)
            .map_err(|error| format!("State offer: {error:?}"))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.push(offer);
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        Ok(())
    }
}
