//! Physical scanout custody selected by checked Source, distinct from a Mask
//! Show pool or renderer capability. No descriptor can construct this claim.
use crate::{StdHost, StdHostConfig};
use conduit_core::{
    resource_offer, BaseEnforcementClass, BaseLifecycle, BaseProviderEntry, BootId, HostId,
    OfferGeneration,
};
use conduit_host_make::HostImage;
use conduit_linear_framebuffer_make::CheckedScanoutInitialization;
use conduit_linux_display::{DisplayHostClaim, InitializedDisplay};

enum BackendPolicy {
    Kmsdrm,
    #[cfg(test)]
    DiagnosticDummy,
}

pub struct SelectedScanoutBase {
    claim: DisplayHostClaim,
    host: HostId,
    boot: BootId,
    generation: OfferGeneration,
    pub(super) entry: BaseProviderEntry,
    pub(super) native_mask: Option<crate::selected_native_mask::SelectedNativeMaskBase>,
}
impl SelectedScanoutBase {
    pub(crate) fn prepare(
        config: &StdHostConfig,
        image: &HostImage,
        parameters: CheckedScanoutInitialization,
        initialized: InitializedDisplay,
    ) -> Result<Self, String> {
        Self::prepare_backend(
            config,
            image,
            parameters,
            initialized,
            BackendPolicy::Kmsdrm,
        )
    }
    #[cfg(test)]
    pub(crate) fn prepare_diagnostic(
        config: &StdHostConfig,
        image: &HostImage,
        parameters: CheckedScanoutInitialization,
        initialized: InitializedDisplay,
    ) -> Result<Self, String> {
        Self::prepare_backend(
            config,
            image,
            parameters,
            initialized,
            BackendPolicy::DiagnosticDummy,
        )
    }
    fn prepare_backend(
        config: &StdHostConfig,
        image: &HostImage,
        parameters: CheckedScanoutInitialization,
        initialized: InitializedDisplay,
        policy: BackendPolicy,
    ) -> Result<Self, String> {
        let manifest = &image.manifest;
        if manifest.source_configuration_id.as_deref() != Some(parameters.configuration_id())
            || !manifest.target.starts_with("std/")
            || config.offer_generation.0 == 0
        {
            return Err("selected scanout Source/IMAGE/Boot mismatch".into());
        }
        let backend = match policy {
            BackendPolicy::Kmsdrm => initialized.is_kmsdrm(),
            #[cfg(test)]
            BackendPolicy::DiagnosticDummy => initialized.backend() == "dummy",
        };
        if !initialized.is_live()
            || !backend
            || initialized.width() != parameters.width()
            || initialized.height() != parameters.height()
            || initialized.window_size() != (parameters.width(), parameters.height())
        {
            return Err("selected scanout requires exact live KMSDRM geometry".into());
        }
        let mut bases = manifest
            .base_selections
            .iter()
            .filter(|base| base.kind == "display/scanout");
        let base = bases.next().ok_or("selected scanout Base is absent")?;
        if bases.next().is_some() || base.driver != "display/linear-framebuffer@1" {
            return Err("selected scanout requires one exact declared driver".into());
        }
        let mut pools = manifest
            .resource_budgets
            .iter()
            .filter(|pool| pool.class == "face/surface");
        let pool = pools
            .next()
            .ok_or("selected scanout surface budget is absent")?;
        if pools.next().is_some() || pool.slots != 1 || pool.bytes < parameters.surface_bytes() {
            return Err(
                "selected scanout requires one finite surface covering actual CPU pixels".into(),
            );
        }
        let entry = BaseProviderEntry {
            base_id: base.id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/scanout/{}",
                config.host_id.as_str(),
                config.boot_id.as_str(),
                initialized.instance_nonce()
            )
            .as_str()
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: "display/linear-framebuffer@1".into(),
            mechanism_family: "display/scanout".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![],
            resources: vec![resource_offer(&pool.id, "face/surface", pool.slots)],
        };
        let claim = initialized
            .claim_host()
            .map_err(|error| format!("selected scanout custody: {error:?}"))?;
        Ok(Self {
            claim,
            host: config.host_id.clone(),
            boot: config.boot_id.clone(),
            generation: config.offer_generation,
            entry,
            native_mask: None,
        })
    }
    pub fn initialized(&self) -> &InitializedDisplay {
        self.claim.initialized()
    }
    pub fn entry(&self) -> &BaseProviderEntry {
        &self.entry
    }
}
impl StdHost {
    /// Borrow only the exact still-live physical Base from this IMAGE/Boot.
    /// This grants no renderer, Show, input, membership or execution authority.
    pub fn ready_selected_scanout(&self) -> Option<&SelectedScanoutBase> {
        self.selected_scanout.as_ref().filter(|scanout| {
            scanout.initialized().is_live()
                && scanout.host == self.advertisement.host_id
                && scanout.boot == self.advertisement.boot_id
                && scanout.generation == self.advertisement.offer_generation
                && self.base_registry.entries().contains(&scanout.entry)
                && self
                    .advertisement
                    .bases
                    .iter()
                    .filter(|base| base.base_id == scanout.entry.base_id)
                    .count()
                    == 1
                && self
                    .advertisement
                    .bases
                    .iter()
                    .any(|base| crate::hosted_display::descriptor_matches(base, &scanout.entry))
                && scanout.entry.resources.iter().all(|resource| {
                    self.advertisement
                        .resources
                        .iter()
                        .filter(|offered| offered.pool_id == resource.pool_id)
                        .eq(std::iter::once(resource))
                })
        })
    }
}

#[cfg(test)]
mod tests;
