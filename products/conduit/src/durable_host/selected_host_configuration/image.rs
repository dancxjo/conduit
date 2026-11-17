//! Verify existing canonical Make artifacts and realize one selected IMAGE.
use super::*;
use conduit_core::{BootId, OfferGeneration};
use conduit_host_make::{verify_configuration_image, verify_image_binding, HostImage};
use conduit_std_host::{hosted_audio::HostedPlaybackSelection, StdHostConfig};
pub(super) fn check_image(
    source: &str,
    image: &HostImage,
) -> Result<CheckedHostConfiguration, String> {
    let checked = super::check(source)?;
    // Make's verifier accepts canonical or pretty HostImage JSON and checks
    // decoded payload/manifest identity. Bound this serialization before any
    // owned encoded Vec or verifier-owned decoded artifact can be created.
    let mut bytes = [0u8; 64 * 1024];
    let length = {
        let mut writer = std::io::Cursor::new(bytes.as_mut_slice());
        serde_json::to_writer(&mut writer, image)
            .map_err(|_| "canonical IMAGE exceeds its finite input frame")?;
        writer.position() as usize
    };
    verify_image_binding(image, &bytes[..length])
        .map_err(|_| "canonical IMAGE payload identity changed")?;
    verify_configuration_image(&checked, image, &conduit_workspace_make::catalog())?;
    Ok(checked)
}
impl Selection {
    pub(in crate::durable_host) fn host_image(&self) -> Option<&HostImage> {
        self.host_image.as_ref()
    }
    pub(in crate::durable_host) fn requires_scanout(&self) -> Result<bool, String> {
        Ok(self
            .checked()?
            .configuration()
            .bases
            .iter()
            .any(|b| b.kind == "display/scanout" && b.initialization.is_some()))
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(in crate::durable_host) fn scanout_parameters(
        &self,
    ) -> Result<Option<conduit_workspace_make::CheckedScanoutInitialization>, String> {
        if !self.requires_scanout()? {
            return Ok(None);
        }
        if self.host_image.is_none() {
            return Err("selected scanout requires its exact canonical --image".into());
        }
        let checked = self.checked()?;
        let native =
            conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(&checked)?;
        if native.input().is_some() {
            return Err(
                "configured native input has no admitted product Show/input consumer".into(),
            );
        }
        conduit_workspace_make::CheckedScanoutInitialization::from_configuration(&checked).map(Some)
    }
    pub(in crate::durable_host) fn prepare_playback(
        &self,
        boot: &BootId,
        generation: OfferGeneration,
    ) -> Result<Option<HostedPlaybackSelection>, String> {
        let checked = self.checked()?;
        let mut declarations = checked
            .configuration()
            .bases
            .iter()
            .filter(|b| b.kind == "audio/speaker" && b.initialization.is_some());
        let Some(base) = declarations.next() else {
            return Ok(None);
        };
        if declarations.next().is_some() {
            return Err("selected IMAGE requires one exact speaker".into());
        }
        playback::initialize_on(base.initialization.as_ref().unwrap(), boot, generation).map(Some)
    }
    pub(in crate::durable_host) fn initialize_image(
        &self,
        config: StdHostConfig,
        #[cfg(all(target_os = "linux", feature = "native-display"))] scanout: Option<
            &mut conduit_linux_display::Scanout<'_>,
        >,
    ) -> Result<StdHost, String> {
        let image = self.host_image.as_ref().ok_or("selected IMAGE is absent")?;
        let checked = self.checked()?;
        for base in &checked.configuration().bases {
            if base.initialization.is_some()
                && !matches!(base.kind.as_str(), "audio/speaker" | "display/scanout")
            {
                return Err(
                    "selected IMAGE cannot initialize this equipment or compiler composition"
                        .into(),
                );
            }
        }
        let selected = self.prepare_playback(&config.boot_id, config.offer_generation)?;
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        if let Some(output) = scanout {
            let parameters =
                conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
                    &checked,
                )?;
            if parameters.input().is_some() {
                return Err(
                    "configured native input has no admitted product Show/input consumer".into(),
                );
            }
            let selected = selected
                .as_ref()
                .ok_or("combined startup member requires its selected speaker")?;
            return StdHost::from_selected_timer_and_native_mask_and_playback_image(
                config,
                image,
                &conduit_workspace_make::catalog(),
                &checked,
                parameters,
                output,
                selected,
            );
        }
        if self.requires_scanout()? {
            return Err("selected scanout requires the native-display product feature and actual initialized output".into());
        }
        if let Some(selected) = selected {
            StdHost::from_selected_timer_and_playback_image(
                config,
                image,
                &conduit_workspace_make::catalog(),
                &checked,
                &selected,
            )
        } else {
            StdHost::from_selected_timer_image(config, image, &conduit_workspace_make::catalog())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str =
        include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit");
    fn artifact() -> (HostImage, Vec<u8>) {
        let checked = super::super::check(SOURCE).unwrap();
        conduit_host_make::build_default_host_image(
            checked.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &conduit_host_make::BuildInputs {
                source_identity: "fixture/product-image".into(),
                toolchain_available: true,
            },
        )
        .unwrap()
    }
    #[test]
    fn supported_make_artifact_is_exactly_the_configure_input_schema() {
        // xtask host build writes these same returned pretty bytes to image.json.
        let (image, bytes) = artifact();
        assert!(bytes.len() <= 64 * 1024);
        let decoded: HostImage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, image);
        check_image(SOURCE, &decoded).unwrap();
        assert!(check_image(&SOURCE.replace("67108864", "67108863"), &decoded).is_err());
        let mut forged = decoded;
        forged.manifest.image_id = "image/forged".into();
        assert!(check_image(SOURCE, &forged).is_err());
    }
    #[test]
    fn oversized_image_refuses_in_fixed_encoder_before_owned_verifier_decode() {
        let (mut image, _) = artifact();
        image.manifest.source_identity = "x".repeat(512 * 1024);
        assert_eq!(
            check_image(SOURCE, &image).unwrap_err(),
            "canonical IMAGE exceeds its finite input frame"
        );
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn explicit_image_selection_preserves_installation_and_refuses_under_active_lock() {
        let (manifest, state) = super::super::super::tests::fixture();
        let initial = super::super::super::install(&manifest, &state).unwrap();
        let source = state.join("selected.host.conduit");
        let image_path = state.join("image.json");
        let (_, bytes) = artifact();
        std::fs::write(&source, SOURCE).unwrap();
        std::fs::write(&image_path, bytes).unwrap();
        let key = state.join("retained-test-key");
        std::fs::write(&key, b"original synthetic key custody").unwrap();
        super::super::configure_with_image(&state, Some(&source), false, Some(&image_path))
            .unwrap();
        let selected = super::super::read_installation(&state.join("installation.json")).unwrap();
        assert_eq!(selected.host_id, initial.host_id);
        assert_eq!(
            serde_json::to_value(&selected.body_state).unwrap(),
            serde_json::to_value(&initial.body_state).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&selected.joined_body_state).unwrap(),
            serde_json::to_value(&initial.joined_body_state).unwrap()
        );
        assert!(selected
            .selected_host_configuration
            .as_ref()
            .unwrap()
            .host_image()
            .is_some());
        assert_eq!(
            std::fs::read(&key).unwrap(),
            b"original synthetic key custody"
        );
        let before = std::fs::read(state.join("installation.json")).unwrap();
        let lock = super::super::owner_lock::acquire(&state).unwrap();
        assert!(super::super::configure_with_image(
            &state,
            Some(&source),
            false,
            Some(&image_path)
        )
        .is_err());
        drop(lock);
        std::fs::write(&image_path, b"{}").unwrap();
        assert!(super::super::configure_with_image(
            &state,
            Some(&source),
            false,
            Some(&image_path)
        )
        .is_err());
        assert_eq!(
            std::fs::read(state.join("installation.json")).unwrap(),
            before
        );
        std::fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}
