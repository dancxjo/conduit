//! Checked physical preparation, funded fixed owners and one final Host ledger.
use super::*;
use crate::{selected_scanout::SelectedScanoutBase, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_linear_framebuffer_make::CheckedNativeMaskInitialization;
pub(crate) struct InitialNativeMask {
    pub(crate) provider: NativeSurfaceProvider,
    pub(crate) entry: BaseProviderEntry,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scanout_cpu_bytes: usize,
    pub(crate) input: Option<conduit_linux_display::NativeInputHostClaim>,
    pub(super) images: Option<images::FundedImages>,
    pub(crate) heap: ImageMemoryLease,
    pub(crate) image_heap_charge: usize,
    pub(crate) runtime_pool_bytes: usize,
    pub(crate) token_bytes: usize,
}
impl InitialNativeMask {
    pub(crate) fn offers(&self, config: &StdHostConfig) -> HostAdvertisement {
        let mut offered = if self.input.is_some() {
            conduit_conduitos_mask_offer::native_host_advertisement(
                &config.host_id,
                &config.boot_id,
                config.offer_generation.0,
                &self.entry,
            )
        } else {
            conduit_conduitos_mask_offer::native_read_only_host_advertisement(
                &config.host_id,
                &config.boot_id,
                config.offer_generation.0,
                &self.entry,
                NativeMaskStorageProfile::Standard,
            )
        };
        offered.capabilities.retain(|offer| {
            offer.kind_id.as_str() == conduit_face::FACE_TEE_KIND
                || self.entry.capabilities.iter().any(|actual| actual == offer)
        });
        offered.resources = self.entry.resources.clone();
        offered
    }
}
impl StdHost {
    pub fn from_selected_timer_and_native_mask_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config, image, catalog, checked, parameters, output, false, None, None, None, None,
        )
    }
    pub fn from_selected_timer_and_native_mask_and_playback_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        selected: &crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            None,
            None,
            None,
            Some(selected),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected_timer_native_mask_with_equipment_and_playback_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        recognizer: Option<crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        input: Option<conduit_linux_display::PreparedNativeInput>,
        images: Option<NativeMaskImageInitialization>,
        selected: &crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            recognizer,
            input,
            images,
            Some(selected),
        )
    }
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
            config, image, catalog, checked, parameters, output, false, recognizer, input, None,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected_timer_native_mask_with_equipment_and_images_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        recognizer: Option<crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        input: Option<conduit_linux_display::PreparedNativeInput>,
        images: NativeMaskImageInitialization,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            recognizer,
            input,
            Some(images),
            None,
        )
    }
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
            None,
            None,
        )
    }
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_native_mask_diagnostic_with_playback(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        input: Option<conduit_linux_display::PreparedNativeInput>,
        selected: &crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            true,
            None,
            input,
            None,
            Some(selected),
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
        image_data: Option<NativeMaskImageInitialization>,
        selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        crate::image_initialization::bounded_image_preflight(&config, image)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected native parameters belong to another Source".into());
        }
        // CPU/compiler/Source construction is separate. Never erase its actual
        // owner or fabricate readiness to make a scanout composition fit.
        if checked.configuration().bases.iter().any(|base| {
            base.kind == "cpu" || base.kind == "cpu/native" || base.kind == "conduit/source"
        }) {
            return Err(
                "selected native initialization does not compose a compiler/CPU Source owner"
                    .into(),
            );
        }
        let memory = crate::kernel_preparation::StagedImageMemory::verified(
            &config, image, checked, catalog,
        )?;
        let playback = selected
            .map(|selected| {
                crate::image_initialization::prepare_playback_on(
                    &config, image, catalog, checked, selected, &memory,
                )
            })
            .transpose()?;
        let recognition = recognizer
            .map(|adapter| {
                crate::image_initialization::InitialWhisper::prepare(
                    &config, image, checked, adapter,
                )
            })
            .transpose()?;
        let width = parameters.scanout().width();
        let height = parameters.scanout().height();
        let (kernel, raster) = SelectedNativeMaskBase::required_bytes(width, height)?;
        let initialized = output.initialized_display()?;
        let nonce = initialized.instance_nonce();
        let instance_bytes = config
            .host_id
            .as_str()
            .len()
            .checked_add(config.boot_id.as_str().len())
            .and_then(|bytes| bytes.checked_add("//scanout/".len()))
            .and_then(|bytes| bytes.checked_add(nonce.checked_ilog10().unwrap_or(0) as usize + 1))
            .ok_or("native instance identity overflow")?;
        let token_bytes = config
            .host_id
            .as_str()
            .len()
            .checked_add(config.boot_id.as_str().len())
            .and_then(|bytes| bytes.checked_add(instance_bytes))
            .and_then(|bytes| bytes.checked_add(parameters.memory().id.len()))
            .and_then(|bytes| bytes.checked_add(RUNTIME_MEMORY_RESOURCE_CLASS.len()))
            .ok_or("native reservation identity overflow")?;
        let runtime_pool_bytes = kernel
            .checked_add(raster)
            .and_then(|bytes| bytes.checked_add(token_bytes))
            .and_then(|bytes| bytes.checked_add(SURFACE.len()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<SelectedScanoutBase>()))
            .and_then(|bytes| {
                bytes.checked_add(input.as_ref().map_or(0, |_| {
                    conduit_linux_display::PreparedNativeInput::owned_backing_bytes()
                }))
            })
            .ok_or("selected native fixed backing overflow")?;
        let image_preparation = match image_data.as_ref() {
            Some(images) => images
                .preparation_bytes
                .checked_add(token_bytes)
                .ok_or("native image reservation overflow")?,
            None => 0,
        };
        if runtime_pool_bytes
            .checked_add(image_preparation)
            .is_none_or(|bytes| bytes > parameters.memory().slots as usize)
        {
            return Err("selected native fixed owners exceed declared runtime pool".into());
        }
        let image_heap_charge = runtime_pool_bytes
            .checked_add(output.cpu_backing_bytes())
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("selected native heap overflow")?;
        // BOTH native and optional image peaks use this SAME counter before any
        // claim/store/factory; speaker charge is already held in this staging.
        let heap = memory.reserve(image_heap_charge)?;
        let images = image_data
            .map(|initial| {
                memory
                    .reserve(image_preparation)
                    .map(|heap| images::FundedImages {
                        initial,
                        heap,
                        token_bytes,
                    })
            })
            .transpose()?;
        if input.is_some() != parameters.input().is_some() {
            return Err("checked native input requires its actual prepared owner".into());
        }
        let input = input
            .map(|input| {
                if !input.matches_display(&initialized) {
                    return Err("prepared input belongs to another/lost display".to_string());
                }
                input
                    .claim_host()
                    .map_err(|error| format!("native input claim:{error:?}"))
            })
            .transpose()?;
        #[cfg(test)]
        let scanout = if diagnostic {
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
        let scanout = {
            if diagnostic {
                return Err("diagnostic display is not a production entrance".into());
            }
            SelectedScanoutBase::prepare(&config, image, parameters.scanout().clone(), initialized)
        };
        let mut scanout = scanout?;
        for budget in [parameters.show(), parameters.memory()] {
            scanout
                .entry
                .resources
                .push(resource_offer(&budget.id, &budget.class, budget.slots));
        }
        scanout
            .entry
            .resources
            .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        let mut issuer = [0; 32];
        getrandom::fill(&mut issuer).map_err(|error| format!("native issuer entropy:{error}"))?;
        // The physical bridge admits exactly its Show possession. Runtime and
        // scanout pools remain independently checked initialized Base resources;
        // they are attached only after that unchanged bridge has validated output.
        let mut show_entry = scanout.entry.clone();
        show_entry
            .resources
            .retain(|pool| pool.class_id.as_str() == conduit_face::SHOW_RESOURCE_CLASS);
        let provider = NativeSurfaceProvider::from_initialized_output(show_entry, issuer, output)
            .map_err(|error| format!("selected native provider:{error:?}"))?;
        let mut entry = scanout.entry.clone();
        let offered = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
            &config.host_id,
            &config.boot_id,
            config.offer_generation.0,
            &entry,
            NativeMaskStorageProfile::Standard,
        );
        entry.capabilities = offered
            .capabilities
            .into_iter()
            .filter(|offer| offer.kind_id.as_str() != conduit_face::FACE_TEE_KIND)
            .collect();
        let expected = [
            "face/renderer-conduitos-component@1",
            "face/conduitos-surface-source@1",
            "face/renderer-conduitos-native@1",
            "face/conduitos-human-input@1",
        ];
        if entry.capabilities.len() != 4
            || !expected.iter().all(|id| {
                entry
                    .capabilities
                    .iter()
                    .any(|offer| offer.implementation.implementation_id.as_str() == *id)
            })
        {
            return Err("native actual Back closure is incomplete".into());
        }
        if let Some(pool) = parameters.input() {
            entry.resources.push(resource_offer(
                &pool.id,
                "conduit.resource/conduitos-human-input@1",
                1,
            ));
            let offered = conduit_conduitos_mask_offer::native_host_advertisement(
                &config.host_id,
                &config.boot_id,
                config.offer_generation.0,
                &entry,
            );
            let interaction = offered
                .capabilities
                .into_iter()
                .find(|offer| offer.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND)
                .ok_or("native input Back absent")?;
            entry
                .capabilities
                .retain(|offer| offer.kind_id.as_str() != conduit_face::FACE_INTERACTION_KIND);
            entry.capabilities.push(interaction);
            entry
                .capabilities
                .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
            entry.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
        }
        scanout.entry = entry.clone();
        let native = InitialNativeMask {
            provider,
            entry,
            width,
            height,
            scanout_cpu_bytes: output.cpu_backing_bytes(),
            input,
            images,
            heap,
            image_heap_charge,
            runtime_pool_bytes,
            token_bytes,
        };
        Self::from_selected_initial_bases(
            config.clone(),
            image,
            catalog,
            crate::image_initialization::InitialBases {
                memory: Some(memory),
                playback,
                recognition,
                scanout: Some(Box::new(scanout)),
                native_mask: Some(native),
            },
        )
    }
}
