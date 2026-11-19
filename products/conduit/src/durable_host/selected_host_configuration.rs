//! Retained checked Host initialization; declarations confer no runtime readiness.
use super::{
    bounded_read, installation_read, owner_lock, read_installation, running_target_id,
    write_json_atomic,
};
use conduit_host_make::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, CheckedHostConfiguration,
};
use conduit_std_host::StdHost;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[path = "selected_host_configuration/image.rs"]
mod image;
#[path = "selected_host_configuration/microphone.rs"]
mod microphone;
#[path = "selected_host_configuration/playback.rs"]
mod playback;
#[path = "selected_host_configuration/regional.rs"]
mod regional;
pub(crate) use regional::viewer_preferences;
#[path = "selected_host_configuration/recognition.rs"]
mod recognition;
const MAXIMUM_SOURCE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    source: String,
    configuration_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_image: Option<conduit_host_make::HostImage>,
}
impl Selection {
    fn from_source(source: &str, target: &str) -> Result<Self, String> {
        let checked = check(source)?;
        require_target(&checked, target)?;
        let source = canonical_host_configuration_conduit(checked.configuration())
            .map_err(|e| format!("canonical Host configuration: {e:?}"))?;
        if source.len() as u64 > MAXIMUM_SOURCE_BYTES {
            return Err("canonical Host configuration exceeds retained bound".into());
        }
        Ok(Self {
            source,
            configuration_id: checked.configuration_id().into(),
            host_image: None,
        })
    }
    pub(super) fn validate_speech_overlap(
        &self,
        speech: Option<&super::selected_speech::Selection>,
    ) -> Result<(), String> {
        if speech.is_some_and(|selection| selection.uses_physical_speaker())
            && self
                .checked()?
                .configuration()
                .bases
                .iter()
                .any(|base| base.kind == "audio/speaker" && base.initialization.is_some())
        {
            return Err("checked speaker conflicts with retained physical speech selection; explicitly remove one selection".into());
        }
        Ok(())
    }
    pub(super) fn checked(&self) -> Result<CheckedHostConfiguration, String> {
        let checked = if let Some(image) = &self.host_image {
            image::check_image(&self.source, image)?
        } else {
            check(&self.source)?
        };
        if checked.configuration_id() != self.configuration_id {
            return Err("retained Host configuration identity changed".into());
        }
        Ok(checked)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        self.checked().map(|_| ())
    }
    pub(super) fn attach_to_fresh_host(&self, host: &mut StdHost) -> Result<(), String> {
        let checked = self.checked()?;
        require_target(&checked, running_target_id()?)?;
        host.set_declared_name(
            conduit_core::HostDeclaredName::new(checked.configuration().name.clone())
                .map_err(|e| e.to_string())?,
        )?;
        let mut initialized = std::collections::BTreeSet::new();
        for base in &checked.configuration().bases {
            let Some(initialization) = &base.initialization else {
                continue;
            };
            if !initialized.insert(&base.kind) {
                continue;
            }
            match base.implementation.as_deref().or_else(|| {
                checked
                    .resolved_bases()
                    .iter()
                    .find(|(kind, _)| kind == &base.kind)
                    .map(|(_, implementation)| implementation.as_str())
            }) {
                Some(
                    implementation @ (conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION
                    | conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION),
                ) => {
                    let adapter = microphone::initialize(initialization, implementation)?;
                    host.attach_microphone(adapter)?;
                }
                // The local viewer initializes its regional Base when the terminal opens.
                Some(regional::IMPLEMENTATION) => {
                    regional::parameters(initialization).map(|_| ())?
                }
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    let selected = playback::initialize(initialization, host)?;
                    host.attach_selected_playback(selected)?;
                }
                #[cfg(not(all(target_os = "linux", feature = "native-display")))]
                Some("display/linear-framebuffer@1") => {
                    return Err(
                        "selected scanout requires the native-display product feature".into(),
                    )
                }
                #[cfg(all(target_os = "linux", feature = "native-display"))]
                Some("display/linear-framebuffer@1") => {
                    conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
                        &checked,
                    )?;
                }
                Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION) => {
                    host.attach_whisper_clip_recognizer(recognition::initialize(initialization)?)?;
                }
                _ => return Err("checked Base has no installed initialization adapter".into()),
            }
        }
        Ok(())
    }
}
fn check(source: &str) -> Result<CheckedHostConfiguration, String> {
    if source.len() as u64 > MAXIMUM_SOURCE_BYTES {
        return Err("Host configuration exceeds retained bound".into());
    }
    let configuration = parse_host_configuration_conduit(source)
        .map_err(|e| format!("Host configuration decode: {e:?}"))?;
    let checked = check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|e| format!("Host configuration refused: {e:?}"))?;
    for base in &checked.configuration().bases {
        if let Some(initialization) = &base.initialization {
            let implementation = checked
                .resolved_bases()
                .iter()
                .find(|(kind, _)| kind == &base.kind)
                .map(|(_, implementation)| implementation.as_str());
            match implementation {
                Some(
                    implementation @ (conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION
                    | conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION),
                ) => microphone::validate(initialization, implementation)?,
                // The local viewer initializes its regional Base when the terminal opens.
                Some(regional::IMPLEMENTATION) => {
                    regional::parameters(initialization).map(|_| ())?
                }
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    playback::validate(initialization)?
                }
                #[cfg(not(all(target_os = "linux", feature = "native-display")))]
                Some("display/linear-framebuffer@1") => {
                    return Err(
                        "selected scanout requires the native-display product feature".into(),
                    )
                }
                #[cfg(all(target_os = "linux", feature = "native-display"))]
                Some("display/linear-framebuffer@1") => {
                    conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
                        &checked,
                    )?;
                }
                Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION) => {
                    recognition::validate(initialization)?;
                }
                _ => return Err("checked Base has no installed initialization adapter".into()),
            }
        }
    }
    Ok(checked)
}
fn require_target(checked: &CheckedHostConfiguration, target: &str) -> Result<(), String> {
    if checked.profile().target.key() != target {
        return Err("checked Host target differs from actual running board".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn configure(
    state_dir: &Path,
    source: Option<&Path>,
    clear: bool,
) -> Result<(), String> {
    configure_with_image(state_dir, source, clear, None)
}
pub(super) fn configure_with_image(
    state_dir: &Path,
    source: Option<&Path>,
    clear: bool,
    image_path: Option<&Path>,
) -> Result<(), String> {
    if source.is_some() == clear || image_path.is_some() && (clear || source.is_none()) {
        return Err("select a Host configuration or explicitly clear it".into());
    }
    let _host = owner_lock::acquire(state_dir)?;
    let _body = owner_lock::body(state_dir)?;
    let path = state_dir.join("installation.json");
    let mut installation = installation_read::read_for_configuration_change(&path)?;
    let selection = if let Some(path) = source {
        if !path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.ends_with(".host.conduit"))
        {
            return Err("Host configuration requires canonical .host.conduit source".into());
        }
        let bytes = bounded_read(path, MAXIMUM_SOURCE_BYTES)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| "Host configuration must be UTF-8")?;
        let mut selection = Selection::from_source(source, running_target_id()?)?;
        if let Some(path) = image_path {
            let bytes = bounded_read(path, 64 * 1024)?;
            let image: conduit_host_make::HostImage = serde_json::from_slice(&bytes)
                .map_err(|_| "invalid canonical Make IMAGE artifact")?;
            image::check_image(source, &image)?;
            selection.host_image = Some(image);
        } else if let Some(prior) = &installation.selected_host_configuration {
            if prior.configuration_id == selection.configuration_id {
                selection.host_image = prior.host_image.clone();
            } else if prior.host_image.is_some() {
                return Err(
                    "changing IMAGE-bound configuration requires an explicit replacement --image"
                        .into(),
                );
            }
        }
        if installation.selected_todo_checkpoint.is_some() {
            return Err(
                "scoped Todo checkpoint Host cannot initialize additional checked equipment".into(),
            );
        }
        Some(selection)
    } else {
        None
    };
    if let Some(selection) = &selection {
        selection.validate_speech_overlap(installation.selected_speech.as_ref())?;
    }
    installation.selected_host_configuration = selection;
    let bytes = serde_json::to_vec_pretty(&installation).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("installed Host selection exceeds retained installation bound".into());
    }
    write_json_atomic(&path, &installation)?;
    read_installation(&path)?;
    println!("retained checked Host configuration for the next Boot");
    Ok(())
}

#[cfg(test)]
#[path = "selected_host_configuration/tests.rs"]
mod tests;
