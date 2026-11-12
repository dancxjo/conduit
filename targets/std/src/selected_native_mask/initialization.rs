use super::*;
use crate::{selected_scanout::SelectedScanoutBase, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_linear_framebuffer_make::CheckedNativeMaskInitialization;

pub(crate) struct InitialNativeMask {
    pub provider: NativeSurfaceProvider,
    pub width: u32,
    pub height: u32,
    pub scanout_cpu_bytes: usize,
    pub input: Option<conduit_linux_display::NativeInputHostClaim>,
}
impl InitialNativeMask {
    pub(crate) fn offers(&self, config: &StdHostConfig) -> HostAdvertisement {
        if self.input.is_some() {
            let mut offer = conduit_conduitos_mask_offer::native_host_advertisement(
                &config.host_id,
                &config.boot_id,
                config.offer_generation.0,
                &self.provider.entry,
            );
            // Preserve actual observed pools; never retain the factory's generic
            // synthesized human-input pool as physical readiness evidence.
            // This initialized entrance prepares the standard renderer and
            // interaction, not the separately configured component renderer.
            // Keep only the exact physical provider closure plus the pure tee.
            offer.capabilities.retain(|offered| {
                offered.kind_id.as_str() == conduit_face::FACE_TEE_KIND
                    || self
                        .provider
                        .entry
                        .capabilities
                        .iter()
                        .any(|actual| actual == offered)
            });
            offer.resources = self.provider.entry.resources.clone();
            if let Some(base) = offer.bases.first_mut() {
                base.capability_ids = self
                    .provider
                    .entry
                    .capabilities
                    .iter()
                    .map(|offer| offer.capability_id.clone())
                    .collect();
            }
            return offer;
        }
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
            self.input,
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
        Self::from_native_mask_backend(
            config, image, catalog, checked, parameters, output, false, None, None,
        )
    }
    /// Retain the actual preinitialized clip recognizer in this initial IMAGE.
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected_timer_and_native_mask_and_recognizer_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        recognizer: crate::hosted_speech_recognition::WhisperSpeechAdapter,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            Some(recognizer),
            None,
        )
    }
    /// Actual prepared SDL input and recognizer are supplied before the single
    /// registry/ledger. Descriptions alone never initialize human input.
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected_timer_native_mask_with_equipment_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        recognizer: Option<crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        input: Option<conduit_linux_display::PreparedNativeInput>,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config, image, catalog, checked, parameters, output, false, recognizer, input,
        )
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
        Self::from_native_mask_backend(
            config, image, catalog, checked, parameters, output, true, None, None,
        )
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_native_mask_input_diagnostic(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        input: conduit_linux_display::PreparedNativeInput,
    ) -> Result<Self, String> {
        // Actual settled SDL input remains mandatory even for CPU diagnostics.
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            true,
            None,
            Some(input),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn from_native_mask_backend(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        diagnostic: bool,
        recognizer: Option<crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        input: Option<conduit_linux_display::PreparedNativeInput>,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected native Mask parameters belong to another Source".into());
        }
        let recognition = recognizer
            .map(|adapter| {
                crate::image_initialization::InitialWhisper::prepare(
                    &config, image, checked, adapter,
                )
            })
            .transpose()?;
        // Deterministic store/budget refusals precede exclusive physical claim.
        // Once claimed, dropping a failed owner correctly revokes that Scanout;
        // it must never be silently reattached after uncertain retirement.
        let (kernel, raster) = SelectedNativeMaskBase::required_bytes(
            parameters.scanout().width(),
            parameters.scanout().height(),
        )?;
        let input_owner_bytes = input.as_ref().map_or(0, |_| {
            conduit_linux_display::PreparedNativeInput::owned_backing_bytes()
        });
        let fixed = kernel
            .checked_add(raster)
            .and_then(|bytes| bytes.checked_add(input_owner_bytes))
            .ok_or("selected native fixed store overflow")?;
        if fixed > parameters.memory().slots as usize {
            return Err("selected native Mask fixed stores exceed declared runtime pool".into());
        }
        let heap = fixed
            .checked_add(output.cpu_backing_bytes())
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("selected native fixed IMAGE heap overflow")?;
        if heap as u64 > image.manifest.bounds.heap_arena_bytes {
            return Err("selected native Mask fixed stores exceed declared IMAGE heap".into());
        }
        let initialized = output.initialized_display()?;
        if input.is_some() != parameters.input().is_some() {
            return Err(
                "checked native input selection requires its actual prepared SDL owner".into(),
            );
        }
        let input = input
            .map(|input| {
                if !input.matches_display(&initialized) {
                    return Err("prepared SDL input belongs to another/lost display".to_string());
                }
                input
                    .claim_host()
                    .map_err(|error| format!("native input claim: {error:?}"))
            })
            .transpose()?;
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
            .filter(|capability| capability.kind_id.as_str() != conduit_face::FACE_TEE_KIND)
            .cloned()
            .collect();
        if provider.entry.capabilities.len() != 3
            || [
                conduit_face::SHOW_RESOURCE_SOURCE_KIND,
                conduit_face::RESOURCE_RENDERER_KIND,
                conduit_face::FACE_INTERACTION_KIND,
            ]
            .into_iter()
            .any(|kind| {
                provider
                    .entry
                    .capabilities
                    .iter()
                    .filter(|offer| offer.kind_id.as_str() == kind)
                    .count()
                    != 1
            })
        {
            return Err(
                "selected native initialized Back closure is incomplete or ambiguous".into(),
            );
        }
        if let Some(pool) = parameters.input() {
            provider.entry.resources.push(conduit_core::resource_offer(
                &pool.id,
                "conduit.resource/conduitos-human-input@1",
                1,
            ));
            let offered = conduit_conduitos_mask_offer::native_host_advertisement(
                &config.host_id,
                &config.boot_id,
                config.offer_generation.0,
                &provider.entry,
            );
            let interaction = offered
                .capabilities
                .into_iter()
                .find(|offer| offer.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND)
                .ok_or("native input Back missing")?;
            provider
                .entry
                .capabilities
                .retain(|offer| offer.kind_id.as_str() != conduit_face::FACE_INTERACTION_KIND);
            provider.entry.capabilities.push(interaction);
            provider
                .entry
                .capabilities
                .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
            provider
                .entry
                .resources
                .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        }
        scanout.entry = provider.entry.clone();
        let intent = InitialNativeMask {
            provider,
            width,
            height,
            scanout_cpu_bytes: output.cpu_backing_bytes(),
            input,
        };
        Self::from_selected_native_initial_bases(
            config,
            image,
            catalog,
            scanout,
            intent,
            recognition,
        )
    }
}
