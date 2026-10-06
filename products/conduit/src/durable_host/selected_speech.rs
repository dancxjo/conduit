//! Persistent, explicit local equipment selection before an installed Boot is
//! published or admitted into a Body. No provider is inferred at service start.
use crate::cli::InstalledSpeechOptions;
use conduit_std_host::{
    hosted_audio::{
        discover_alsa_playback, AlsaPlaybackObservation, ExplicitPlaybackAuthorization,
        HostedPlaybackSelection,
    },
    hosted_speech_synthesis::EspeakDiscovery,
    hosted_wav_artifact::WavArtifactSelection,
    StdHost,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    card_id: String,
    device: u16,
    speaker_base_identity: String,
    executable: PathBuf,
    data_root: PathBuf,
    voice: String,
    engine_dependencies: Vec<PathBuf>,
    provider_sha256: String,
}

pub(super) enum Change {
    Preserve,
    Replace(Selection),
    Remove,
}

#[derive(Clone)]
pub(crate) struct AttachedEquipment {
    pub(crate) playback: HostedPlaybackSelection,
    pub(crate) authorization: ExplicitPlaybackAuthorization,
    pub(crate) provider_sha256: String,
    #[cfg(test)]
    pub(crate) before_play: Option<std::sync::Arc<std::sync::Barrier>>,
}

impl AttachedEquipment {
    pub(crate) fn matches(&self, host: &StdHost) -> bool {
        host.selected_spoken_equipment_matches(&self.playback, &self.provider_sha256)
    }
}

impl Selection {
    pub(super) fn validate(&self) -> Result<(), String> {
        self.validate_inputs()?;
        if self.speaker_base_identity.is_empty()
            || self.speaker_base_identity.len() > 256
            || self.provider_sha256.len() != 64
            || !self
                .provider_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("installed selected speech identity violates its finite bounds".into());
        }
        Ok(())
    }

    fn validate_inputs(&self) -> Result<(), String> {
        if self.card_id.is_empty()
            || self.card_id.len() > 128
            || self.voice.is_empty()
            || self.voice.len() > 64
            || !self
                .voice
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || self.engine_dependencies.is_empty()
            || self.engine_dependencies.len() > 16
            || self.executable.as_os_str().is_empty()
            || self.data_root.as_os_str().is_empty()
            || self.executable.as_os_str().len() > 4096
            || self.data_root.as_os_str().len() > 4096
            || self
                .engine_dependencies
                .iter()
                .any(|path| path.as_os_str().is_empty() || path.as_os_str().len() > 4096)
        {
            return Err(
                "installed selected speech configuration violates its finite bounds".into(),
            );
        }
        Ok(())
    }

    fn reviewed_with(
        mut self,
        speaker_base_identity: String,
        provider_sha256: String,
    ) -> Result<Self, String> {
        self.speaker_base_identity = speaker_base_identity;
        self.provider_sha256 = provider_sha256;
        self.validate()?;
        Ok(self)
    }

    /// This runs before runtime.json publication and before Body ownership.
    /// Discovery does not open a PCM handle; the selected Back rechecks the
    /// actual device when a Play starts.
    pub(super) fn attach_to_fresh_host(
        &self,
        host: &mut StdHost,
    ) -> Result<AttachedEquipment, String> {
        self.attach_to_fresh_host_with_optional_artifact(host, None)
    }

    /// Select the speaker, voice, and one create-new WAV destination before
    /// this Boot is advertised. The destination can complete once per Boot;
    /// a later spoken Show requires a new Boot or an admitted per-Play output
    /// selection, never replacement of an already published artifact.
    pub(super) fn attach_to_fresh_host_with_artifact(
        &self,
        host: &mut StdHost,
        destination: &Path,
    ) -> Result<AttachedEquipment, String> {
        if destination.exists() {
            return Err("selected spoken artifact destination already exists".into());
        }
        let offer = host.advertisement();
        let artifact =
            WavArtifactSelection::new(destination, offer.boot_id.clone(), offer.offer_generation)?;
        self.attach_to_fresh_host_with_optional_artifact(host, Some(artifact))
    }

    fn attach_to_fresh_host_with_optional_artifact(
        &self,
        host: &mut StdHost,
        artifact: Option<WavArtifactSelection>,
    ) -> Result<AttachedEquipment, String> {
        self.validate()?;
        let observation = observe_speaker(&self.card_id, self.device)?;
        if observation.base_identity != self.speaker_base_identity {
            return Err(
                "configured speaker observation identity changed; reselect equipment".into(),
            );
        }
        let offered = host.advertisement().clone();
        let playback = HostedPlaybackSelection::from_observation(
            observation,
            offered.boot_id.clone(),
            offered.offer_generation,
        )
        .with_bounded_speech_queue();
        let discovery = EspeakDiscovery::inspect(
            &self.executable,
            &self.data_root,
            &self.voice,
            &self.engine_dependencies,
        )
        .map_err(|error| format!("configured eSpeak provider refused: {error:?}"))?;
        let provider_sha256 = discovery.provider_sha256.clone();
        if provider_sha256 != self.provider_sha256 {
            return Err("configured eSpeak provider content changed; reselect equipment".into());
        }
        let adapter = discovery
            .initialize(
                offered.host_id.clone(),
                offered.boot_id.clone(),
                offered.offer_generation,
                "grant/installed-selected-speech".into(),
                Duration::from_secs(30),
            )
            .map_err(|error| format!("initialize configured eSpeak provider: {error:?}"))?;
        host.attach_selected_playback(playback.clone())?;
        if let Some(artifact) = artifact {
            host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
        } else {
            host.attach_espeak_speech_for_selected_playback(adapter)?;
        }
        Ok(AttachedEquipment {
            playback,
            authorization: ExplicitPlaybackAuthorization::new(&format!(
                "grant/installed-selected-speech/{}",
                offered.boot_id.as_str()
            ))?,
            provider_sha256,
            #[cfg(test)]
            before_play: None,
        })
    }
}

pub(super) fn change(options: InstalledSpeechOptions) -> Result<Change, String> {
    if options.without_selected_speech {
        return Ok(Change::Remove);
    }
    if !options.selected_speech {
        return Ok(Change::Preserve);
    }
    let selection = Selection {
        card_id: options
            .speaker_card
            .ok_or("selected speech needs a speaker card")?,
        device: options
            .speaker_device
            .ok_or("selected speech needs a speaker device")?,
        speaker_base_identity: String::new(),
        executable: options
            .speech_executable
            .ok_or("selected speech needs an executable")?,
        data_root: options
            .speech_data
            .ok_or("selected speech needs voice data")?,
        voice: options.speech_voice.unwrap_or_else(|| "en-us".into()),
        engine_dependencies: options.speech_engine,
        provider_sha256: String::new(),
    };
    selection.validate_inputs()?;
    let observation = observe_speaker(&selection.card_id, selection.device)?;
    let discovery = EspeakDiscovery::inspect(
        &selection.executable,
        &selection.data_root,
        &selection.voice,
        &selection.engine_dependencies,
    )
    .map_err(|error| format!("review selected eSpeak provider: {error:?}"))?;
    Ok(Change::Replace(selection.reviewed_with(
        observation.base_identity,
        discovery.provider_sha256,
    )?))
}

fn observe_speaker(card_id: &str, device: u16) -> Result<AlsaPlaybackObservation, String> {
    let observations = discover_alsa_playback()
        .map_err(|error| format!("discover configured speaker: {error}"))?;
    let mut matching = observations
        .into_iter()
        .filter(|item| item.card_id == card_id && item.device == device);
    let observation = matching
        .next()
        .ok_or("configured speaker is absent from current ALSA discovery")?;
    if matching.next().is_some() {
        return Err("configured speaker observation is ambiguous".into());
    }
    Ok(observation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_selection_pins_bare_provider_digest_and_speaker_identity() {
        let unreviewed = Selection {
            card_id: "card".into(),
            device: 0,
            speaker_base_identity: String::new(),
            executable: "/bin/espeak-ng".into(),
            data_root: "/data/espeak-ng-data".into(),
            voice: "en-us".into(),
            engine_dependencies: vec!["/lib/libespeak-ng.so.1".into()],
            provider_sha256: String::new(),
        };
        let reviewed = unreviewed
            .clone()
            .reviewed_with("physical-card".into(), "a".repeat(64))
            .unwrap();
        assert_eq!(reviewed.speaker_base_identity, "physical-card");
        assert_eq!(reviewed.provider_sha256, "a".repeat(64));
        assert!(unreviewed
            .reviewed_with("physical-card".into(), format!("sha256:{}", "a".repeat(64)))
            .is_err());
    }
}
