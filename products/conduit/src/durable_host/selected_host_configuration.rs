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

#[path = "selected_host_configuration/creche.rs"]
mod creche;
#[path = "selected_host_configuration/image.rs"]
mod image;
#[path = "selected_host_configuration/microphone.rs"]
mod microphone;
#[path = "selected_host_configuration/playback.rs"]
mod playback;
#[path = "selected_host_configuration/recognition.rs"]
mod recognition;
#[cfg(target_os = "linux")]
#[path = "selected_host_configuration/text_file.rs"]
mod text_file;
pub(crate) use image::check_seed_image;
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
    #[cfg(test)]
    pub(super) fn require_timer_image_initialization(&self) -> Result<(), String> {
        self.require_image_initialization(false)
    }
    #[cfg(test)]
    pub(super) fn require_image_initialization(
        &self,
        scanout_prepared: bool,
    ) -> Result<(), String> {
        self.require_prepared_image_initialization(scanout_prepared, false)
    }
    pub(super) fn require_prepared_image_initialization(
        &self,
        scanout_prepared: bool,
        source_prepared: bool,
    ) -> Result<(), String> {
        self.require_image_equipment_initialization(scanout_prepared, source_prepared, false)
    }
    pub(super) fn require_image_equipment_initialization(
        &self,
        scanout_prepared: bool,
        source_prepared: bool,
        recognizer_prepared: bool,
    ) -> Result<(), String> {
        for base in &self.checked()?.configuration().bases {
            if base.initialization.is_some()
                && !(scanout_prepared && base.kind == "display/scanout")
                && !(recognizer_prepared && base.kind == "speech/recognizer")
                && !(source_prepared && base.kind == conduit_std_offers::SOURCE_CHECK_BASE_KIND)
                && !(source_prepared && base.kind == "creche/encounter")
                && base.kind != conduit_std_offers::DURABLE_TEXT_BASE_KIND
                && base.kind != conduit_workspace_make::CPU_BASE_KIND
            {
                return Err(
                    "selected IMAGE cannot initialize this physical equipment or Crèche provider"
                        .into(),
                );
            }
        }
        Ok(())
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(super) fn scanout_parameters(
        &self,
    ) -> Result<Option<conduit_workspace_make::CheckedScanoutInitialization>, String> {
        let checked = self.checked()?;
        if !checked
            .configuration()
            .bases
            .iter()
            .any(|base| base.kind == "display/scanout" && base.initialization.is_some())
        {
            return Ok(None);
        }
        if self.host_image.is_none() {
            return Err("selected scanout requires the Seed's checked IMAGE".into());
        }
        conduit_workspace_make::CheckedScanoutInitialization::from_configuration(&checked).map(Some)
    }
    pub(super) fn has_cpu_initialization(&self) -> Result<bool, String> {
        let checked = self.checked()?;
        if !checked
            .configuration()
            .bases
            .iter()
            .any(|base| base.kind == conduit_workspace_make::CPU_BASE_KIND)
        {
            return Ok(false);
        }
        conduit_workspace_make::CheckedCpuInitialization::from_configuration(&checked)?;
        Ok(true)
    }
    #[cfg(target_os = "linux")]
    pub(super) fn text_file_parameters(
        &self,
    ) -> Result<Option<conduit_workspace_make::CheckedTextFileInitialization>, String> {
        text_file::parameters(&self.checked()?)
    }
    pub(super) fn host_image(&self) -> Option<&conduit_host_make::HostImage> {
        self.host_image.as_ref()
    }
    pub(super) fn configuration_id(&self) -> &str {
        &self.configuration_id
    }
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
        let checked = match &self.host_image {
            Some(image) => check_seed_image(&self.source, image)?,
            None => check(&self.source)?,
        };
        if checked.configuration_id() != self.configuration_id {
            return Err("retained Host configuration identity changed".into());
        }
        Ok(checked)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        self.checked().map(|_| ())
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(super) fn prepare_recognizer(
        &self,
    ) -> Result<Option<conduit_std_host::hosted_speech_recognition::WhisperSpeechAdapter>, String>
    {
        let checked = self.checked()?;
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == "speech/recognizer" && base.initialization.is_some());
        let Some(base) = bases.next() else {
            return Ok(None);
        };
        if bases.next().is_some() {
            return Err("selected IMAGE requires one exact recognizer".into());
        }
        recognition::initialize(
            base.initialization
                .as_ref()
                .ok_or("recognizer lost initialization")?,
        )
        .map(Some)
    }
    pub(super) fn prepare_creche(
        &self,
    ) -> Result<Option<conduit::creche_encounter::CrecheEncounterFactory>, String> {
        let checked = self.checked()?;
        require_target(&checked, running_target_id()?)?;
        for base in &checked.configuration().bases {
            if base.kind == "creche/encounter" {
                let Some(initialization) = base.initialization.as_ref() else {
                    continue;
                };
                return creche::initialize(initialization).map(Some);
            }
        }
        Ok(None)
    }
    #[cfg(test)]
    pub(super) fn attach_to_fresh_host(&self, host: &mut StdHost) -> Result<(), String> {
        self.attach_to_prepared_host(host, None)
    }
    pub(super) fn attach_to_prepared_host(
        &self,
        host: &mut StdHost,
        creche_factory: Option<&conduit::creche_encounter::CrecheEncounterFactory>,
    ) -> Result<(), String> {
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
                Some(conduit_body_make::deployment_runtime_catalog::CRECHE_IMPLEMENTATION) => {
                    let factory = creche_factory
                        .ok_or("Crèche provider was not prepared before Host construction")?;
                    if !host
                        .advertisement()
                        .resources
                        .contains(&factory.resource_offer())
                        || !host.advertisement().capabilities.contains(
                            &conduit_body_make::deployment_runtime_catalog::creche_std_offer(),
                        )
                    {
                        return Err(
                            "Crèche prepared provider differs from initial Host advertisement"
                                .into(),
                        );
                    }
                }
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    let selected = playback::initialize(initialization, host)?;
                    host.attach_selected_playback(selected)?;
                }
                Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION) => {
                    host.attach_whisper_clip_recognizer(recognition::initialize(initialization)?)?;
                }
                Some(conduit_workspace_make::CPU_IMPLEMENTATION) => {
                    return Err(
                        "selected CPU profile requires its checked IMAGE before Boot".into(),
                    );
                }
                Some(
                    conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION
                    | conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION,
                ) => {
                    return Err(
                        "immutable Text residence requires its checked IMAGE before Boot".into(),
                    );
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
                Some(conduit_body_make::deployment_runtime_catalog::CRECHE_IMPLEMENTATION) => {
                    creche::validate(initialization)?
                }
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    playback::validate(initialization)?
                }
                Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION) => {
                    recognition::validate(initialization)?;
                }
                Some(conduit_std_offers::SOURCE_CHECK_IMPLEMENTATION) => {
                    // Validation only: the original selected IMAGE factory must
                    // still initialize and retain the actual compiler provider.
                    conduit_workspace_make::CheckedSourceCheckInitialization::from_configuration(
                        &checked,
                    )?;
                }
                Some(conduit_workspace_make::CPU_IMPLEMENTATION) => {
                    conduit_workspace_make::CheckedCpuInitialization::from_configuration(&checked)?;
                }
                #[cfg(target_os = "linux")]
                Some(
                    conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION
                    | conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION,
                ) => {
                    text_file::parameters(&checked)?;
                }
                Some("display/linear-framebuffer@1") => {
                    conduit_workspace_make::CheckedScanoutInitialization::from_configuration(
                        &checked,
                    )?;
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

pub(super) fn configure(
    state_dir: &Path,
    source: Option<&Path>,
    clear: bool,
) -> Result<(), String> {
    if source.is_some() == clear {
        return Err("select a Host configuration or explicitly clear it".into());
    }
    let _host = owner_lock::acquire(state_dir)?;
    let _body = owner_lock::body(state_dir)?;
    let path = state_dir.join("installation.json");
    let installation = installation_read::read_for_configuration_change(&path)?;
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
        if let Some(prior) = &installation.selected_host_configuration {
            if prior.configuration_id == selection.configuration_id {
                // Reselecting the same source must retain its construction evidence.
                prior.validate()?;
                selection.host_image = prior.host_image.clone();
            } else if prior.host_image.is_some() {
                return Err("changing Seed-bound Host source requires a replacement Seed".into());
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
    retain_selection(state_dir, installation, selection)?;
    println!("retained checked Host configuration for the next Boot");
    Ok(())
}

/// Only the fresh, stopped Seed installer uses this source-bearing entrance.
pub(super) fn select_seed_source(
    state_dir: &Path,
    source: &str,
    expected_id: &str,
    image: &conduit_host_make::HostImage,
) -> Result<(), String> {
    let _host = owner_lock::acquire(state_dir)?;
    let _body = owner_lock::body(state_dir)?;
    let installation =
        installation_read::read_for_configuration_change(&state_dir.join("installation.json"))?;
    if installation.body_state.is_some()
        || installation.joined_body_state.is_some()
        || installation.selected_host_configuration.is_some()
        || installation.selected_speech.is_some()
        || installation.selected_model.is_some()
        || installation.selected_todo_checkpoint.is_some()
    {
        return Err("Seed configuration requires a fresh stopped Host".into());
    }
    let mut selection = Selection::from_source(source, running_target_id()?)?;
    selection.host_image = Some(image.clone());
    selection.validate()?;
    if selection.configuration_id != expected_id {
        return Err("Seed configuration identity changed before installation".into());
    }
    retain_selection(state_dir, installation, Some(selection))
}

fn retain_selection(
    state_dir: &Path,
    mut installation: super::Installation,
    selection: Option<Selection>,
) -> Result<(), String> {
    if let Some(selection) = &selection {
        selection.validate_speech_overlap(installation.selected_speech.as_ref())?;
    }
    installation.selected_host_configuration = selection;
    let bytes = serde_json::to_vec_pretty(&installation).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("installed Host selection exceeds retained installation bound".into());
    }
    let path = state_dir.join("installation.json");
    write_json_atomic(&path, &installation)?;
    read_installation(&path)?;
    Ok(())
}

#[cfg(test)]
#[path = "selected_host_configuration/tests.rs"]
mod tests;

/// Bind a new checked IMAGE to the unchanged retained Host source before release commit.
pub(super) fn replacement_image(
    selection: Option<Selection>,
    image_path: Option<&Path>,
    release_source: &str,
) -> Result<Option<Selection>, String> {
    let Some(mut selection) = selection else {
        if image_path.is_some() {
            return Err("replacement IMAGE requires retained checked Host source".into());
        }
        return Ok(None);
    };
    if let Some(path) = image_path {
        let image: conduit_host_make::HostImage =
            serde_json::from_slice(&bounded_read(path, 64 * 1024)?)
                .map_err(|error| format!("replacement Host IMAGE: {error}"))?;
        check_seed_image(&selection.source, &image)?;
        selection.host_image = Some(image);
    }
    selection.validate()?;
    if selection
        .host_image
        .as_ref()
        .is_some_and(|image| image.manifest.source_identity != release_source)
    {
        return Err(
            "release update requires a checked replacement IMAGE from the same source".into(),
        );
    }
    Ok(Some(selection))
}
