//! Retained checked Host initialization; declarations confer no runtime readiness.
use super::{
    bounded_read, installation_read, owner_lock, read_installation, running_target_id,
    write_json_atomic,
};
use conduit_host_make::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, CheckedHostConfiguration,
};
use conduit_std_host::{StdHost, StdHostComposition};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[path = "selected_host_configuration/microphone.rs"]
mod microphone;
#[path = "selected_host_configuration/playback.rs"]
mod playback;
#[path = "selected_host_configuration/recognition.rs"]
mod recognition;
const MAXIMUM_SOURCE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    source: String,
    configuration_id: String,
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
    fn checked(&self) -> Result<CheckedHostConfiguration, String> {
        let checked = check(&self.source)?;
        if checked.configuration_id() != self.configuration_id {
            return Err("retained Host configuration identity changed".into());
        }
        Ok(checked)
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        self.checked().map(|_| ())
    }
    /// Choose software before constructing advertisement, ledger or providers.
    /// An absent purpose preserves the caller's existing composition. An explicit
    /// empty purpose requests the existing base-only construction baseline.
    pub(super) fn composition(
        &self,
        fallback: StdHostComposition,
    ) -> Result<StdHostComposition, String> {
        let checked = self.checked()?;
        require_target(&checked, running_target_id()?)?;
        match &checked.configuration().software {
            None => Ok(fallback),
            Some(purpose) if purpose.capabilities.is_empty() => Ok(StdHostComposition::minimal()),
            Some(_) => Err("installed Host cannot construct the selected software purpose".into()),
        }
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
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    let selected = playback::initialize(initialization, host)?;
                    host.attach_selected_playback(selected)?;
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
    if checked
        .configuration()
        .software
        .as_ref()
        .is_some_and(|purpose| !purpose.capabilities.is_empty())
    {
        return Err("installed Host cannot construct the selected software purpose".into());
    }
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
                Some(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION) => {
                    playback::validate(initialization)?
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
        let selection = Selection::from_source(source, running_target_id()?)?;
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
