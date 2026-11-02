use super::*;
use crate::{selected_scanout::SelectedScanoutBase, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_linear_framebuffer_make::CheckedNativeMaskInitialization;

pub(crate) struct InitialNativeMask {
    pub provider: NativeSurfaceProvider,
    pub width: u32,
    pub height: u32,
    pub scanout_cpu_bytes: usize,
}
impl InitialNativeMask {
    pub(crate) fn offers(&self, config: &StdHostConfig) -> HostAdvertisement {
        conduit_conduitos_mask_offer::native_read_only_host_advertisement(
            &config.host_id,
            &config.boot_id,
            config.offer_generation.0,
            &self.provider.entry,
            NativeMaskStorageProfile::Standard,
        )
    }
    pub(crate) fn initialize(self, host: &mut StdHost) -> Result<SelectedNativeMaskBase, String> {
        SelectedNativeMaskBase::initialize(
            host,
            self.provider,
            self.width,
            self.height,
            self.scanout_cpu_bytes,
        )
    }
}
impl StdHost {
    /// Checked physical output plus actual preallocated standard read-only Mask
    /// stores. Membership, owner route, Plan and Show remain separate admissions.
    pub fn from_selected_timer_and_native_mask_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(config, image, catalog, checked, parameters, output, false)
    }
    #[cfg(test)]
    pub(crate) fn from_native_mask_diagnostic(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(config, image, catalog, checked, parameters, output, true)
    }
    fn from_native_mask_backend(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        diagnostic: bool,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected native Mask parameters belong to another Source".into());
        }
        let initialized = output.initialized_display()?;
        #[cfg(test)]
        let prepared = if diagnostic {
            SelectedScanoutBase::prepare_diagnostic(
                &config,
                image,
                parameters.scanout().clone(),
                initialized,
            )
        } else {
            SelectedScanoutBase::prepare(&config, image, parameters.scanout().clone(), initialized)
        };
        #[cfg(not(test))]
        let prepared = {
            if diagnostic {
                return Err("diagnostic display is not a production entrance".into());
            }
            SelectedScanoutBase::prepare(&config, image, parameters.scanout().clone(), initialized)
        };
        let mut scanout = prepared?;
        let width = parameters.scanout().width();
        let height = parameters.scanout().height();
        for budget in [parameters.show(), parameters.memory()] {
            scanout
                .entry
                .resources
                .push(resource_offer(&budget.id, &budget.class, budget.slots));
        }
        scanout
            .entry
            .resources
            .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        let mut issuer = [0; 32];
        getrandom::fill(&mut issuer)
            .map_err(|error| format!("selected native issuer entropy: {error}"))?;
        let mut provider =
            NativeSurfaceProvider::from_initialized_output(scanout.entry.clone(), issuer, output)
                .map_err(|error| format!("selected actual native provider: {error:?}"))?;
        let offer = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
            &config.host_id,
            &config.boot_id,
            config.offer_generation.0,
            &provider.entry,
            NativeMaskStorageProfile::Standard,
        );
        provider.entry.capabilities = offer
            .capabilities
            .iter()
            .filter(|capability| {
                capability.kind_id.as_str() == conduit_face::SHOW_RESOURCE_SOURCE_KIND
            })
            .cloned()
            .collect();
        if provider.entry.capabilities.len() != 1 {
            return Err("selected native Source capability is ambiguous".into());
        }
        scanout.entry = provider.entry.clone();
        let intent = InitialNativeMask {
            provider,
            width,
            height,
            scanout_cpu_bytes: output.cpu_backing_bytes(),
        };
        Self::from_selected_native_initial_bases(config, image, catalog, scanout, intent)
    }
}
