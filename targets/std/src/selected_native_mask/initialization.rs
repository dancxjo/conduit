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
        Self::from_native_mask_backend(
            config, image, catalog, checked, parameters, output, false, None,
        )
    }
    /// Preserve the same selected SDL/Mask owners while initializing one checked
    /// speaker on their final IMAGE counter. Membership and grants are separate.
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected_timer_and_native_mask_and_playback_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        playback: &crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            Some(playback),
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
            config, image, catalog, checked, parameters, output, true, None,
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
        playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected native Mask parameters belong to another Source".into());
        }
        // Deterministic store/budget refusals precede exclusive physical claim.
        // Once claimed, dropping a failed owner correctly revokes that Scanout;
        // it must never be silently reattached after uncertain retirement.
        let (kernel, raster) = SelectedNativeMaskBase::required_bytes(
            parameters.scanout().width(),
            parameters.scanout().height(),
        )?;
        let fixed = kernel
            .checked_add(raster)
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
        scanout.entry = provider.entry.clone();
        let intent = InitialNativeMask {
            provider,
            width,
            height,
            scanout_cpu_bytes: output.cpu_backing_bytes(),
        };
        Self::from_selected_native_initial_bases(
            config,
            image,
            catalog,
            scanout,
            intent,
            playback.map(|p| (checked, p)),
        )
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;
    use crate::hosted_audio::{
        AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection,
    };
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    use std::sync::atomic::Ordering;

    #[test]
    fn native_mask_and_startup_speaker_share_the_original_image_counter() {
        // CPU/SDL diagnostics only: this is not KMSDRM or audible-output proof.
        assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video
            .window("combined Mask/speaker diagnostic", 640, 480)
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        let textures = canvas.texture_creator();
        let mut output =
            conduit_linux_display::Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
        let source = include_str!("../../../profiles/raspberry-pi-3-tick-workset.host.conduit").replace("  limits =", &format!(r#"  base = {{kind: "display/scanout", implementation: "display/linear-framebuffer@1", initialization: {{backend: "KMSDRM", width: 640, height: 480}}}}
  base = {{kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {{executable_sha256: "{}", card_id: "Fixture", device: 0, base_identity: "fixture/speaker"}}}}
  capability = {{kind: "sound/startup-chime", pinned_implementation: "std/kernel-startup-chime@1"}}
  capability = {{kind: "body/wake", pinned_implementation: "std/kernel-body-wake@1"}}
  need = {{id: "display/main", class: "face/surface", slots: 1, bytes: 1228800}}
  need = {{id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}}
  need = {{id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: 8388608, bytes: 8388608}}
  need = {{id: "std/audio/alsa/fixture/speaker/card-Fixture/device-0", class: "conduit.resource/audio-playback-alsa-hw@1", slots: 1, bytes: 1}}
  limits ="#, "ab".repeat(32)));
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked = check_host_configuration(
            parse_host_configuration_conduit(&source).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let (image, _) = build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: "diagnostic/native-mask-and-speaker".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        let config = StdHostConfig {
            host_id: "diagnostic/combined".into(),
            boot_id: "diagnostic/boot".into(),
            offer_generation: OfferGeneration(7),
        };
        let selected = HostedPlaybackSelection::deterministic_fake(
            AlsaPlaybackObservation {
                card_index: 999,
                card_id: "Fixture".into(),
                card_name: "Fixture".into(),
                device: 0,
                device_name: "PCM".into(),
                base_identity: "fixture/speaker".into(),
            },
            config.boot_id.clone(),
            config.offer_generation,
            FakePlaybackBehavior::Success,
        )
        .with_executable_sha256("ab".repeat(32))
        .unwrap();
        crate::hosted_audio::startup_chime::reset_test_attempts();
        let mut host = StdHost::from_native_mask_backend(
            config,
            &image,
            &catalog,
            &checked,
            CheckedNativeMaskInitialization::from_configuration(&checked).unwrap(),
            &mut output,
            true,
            Some(&selected),
        )
        .unwrap();
        let used = host.kernel_resources.image_memory_observer();
        let display_charge = host
            .ready_selected_native_mask()
            .unwrap()
            .fixed_backing()
            .image_heap_charge;
        assert!(used.load(Ordering::Acquire) > display_charge);
        assert!(host
            .playback_selection()
            .unwrap()
            .same_physical_selection(&selected));
        assert!(host.advertisement().capabilities.iter().any(|o| o
            .implementation
            .implementation_id
            .as_str()
            == conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION));
        assert!(output.initialized_display().unwrap().is_live());
        assert_eq!(
            crate::hosted_audio::startup_chime::test_attempts(),
            0,
            "initialization never plays a cue"
        );
        drop(host);
        assert_eq!(
            used.load(Ordering::Acquire),
            crate::kernel_preparation::StagedImageMemory::counter_backing_bytes()
        );
    }
}
