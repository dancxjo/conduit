//! Trusted provider integration with the existing Host resource ledger.
use crate::{ProviderResourceReservation, StdHost};
use conduit_core::{BaseInstanceId, ResourceBinding};

impl StdHost {
    /// Atomically reserve finite simple-unit pools from one actual initialized
    /// provider. This accounting grants no platform effect or Line authority.
    pub fn reserve_provider_resources(
        &mut self,
        base: &BaseInstanceId,
        generation: u64,
        bindings: &[ResourceBinding],
    ) -> Result<ProviderResourceReservation, String> {
        self.kernel_resources.reserve_provider_resources(
            &self.advertisement,
            &self.base_registry,
            base,
            generation,
            bindings,
        )
    }
    /// Release exact provider custody after actual retirement; refusal returns
    /// the unchanged token. Dropping it never releases its charges.
    ///
    /// # Safety
    /// All actual effects and workers charged to this token must be closed or
    /// joined. Cancellation intent and transport loss do not prove retirement.
    #[allow(clippy::result_large_err)]
    pub unsafe fn release_provider_resources(
        &mut self,
        token: ProviderResourceReservation,
    ) -> Result<(), (ProviderResourceReservation, String)> {
        self.kernel_resources
            .release_provider_resources(&self.base_registry, token)
    }
}
