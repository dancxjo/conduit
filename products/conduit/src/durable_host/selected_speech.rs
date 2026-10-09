//! Persistent, explicit local equipment selection before an installed Boot is
//! published or admitted into a Body. No provider is inferred at service start.
use crate::cli::InstalledSpeechOptions;
use conduit_plot::rust_binding::NativeRustBinding;
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    artifact_only: bool,
    card_id: String,
    device: u16,
    speaker_base_identity: String,
    executable: PathBuf,
    data_root: PathBuf,
    voice: String,
    engine_dependencies: Vec<PathBuf>,
    provider_sha256: String,
    #[serde(default)]
    language_coverage: Option<Vec<u8>>,
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
    pub(crate) realization_properties: Vec<conduit_core::StructuredConfigurationValue>,
    #[cfg(test)]
    pub(crate) before_play: Option<std::sync::Arc<std::sync::Barrier>>,
}

impl AttachedEquipment {
    pub(crate) fn advance_offer_generation(&mut self, host: &StdHost) -> Result<(), String> {
        let advertisement = host.advertisement();
        if self.playback.boot_id != advertisement.boot_id {
            return Err("selected speaker Boot changed during offer transition".into());
        }
        let prior = self.playback.offer_generation;
        self.playback.offer_generation = advertisement.offer_generation;
        if !self.matches(host) {
            self.playback.offer_generation = prior;
            return Err("selected speech equipment differs after offer transition".into());
        }
        Ok(())
    }

    pub(crate) fn matches(&self, host: &StdHost) -> bool {
        host.selected_spoken_equipment_matches(
            &self.playback,
            &self.provider_sha256,
            &self.realization_properties,
        )
    }
}

impl Selection {
    pub(super) fn validate(&self) -> Result<(), String> {
        self.validate_identity()?;
        self.coverage()?;
        Ok(())
    }

    /// Only explicit replacement/removal may supersede missing legacy coverage.
    /// Existing identity and any supplied declaration remain validated.
    pub(super) fn validate_for_reselection(&self) -> Result<(), String> {
        self.validate_identity()?;
        if self.language_coverage.is_some() {
            self.coverage()?;
        }
        Ok(())
    }

    fn validate_identity(&self) -> Result<(), String> {
        self.validate_inputs()?;
        if (!self.artifact_only && self.speaker_base_identity.is_empty())
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

    fn coverage(&self) -> Result<conduit_language::LanguageCoverage, String> {
        let bytes = self
            .language_coverage
            .as_ref()
            .ok_or("retained speech selection has no Language coverage; reselect equipment")?;
        if bytes.len() > conduit_core::MAXIMUM_REALIZATION_PROPERTY_BYTES {
            return Err("retained speech Language coverage exceeds its finite bound".into());
        }
        let coverage = conduit_language::LanguageCoverage::decode(bytes)
            .map_err(|error| format!("retained Language coverage: {error:?}"))?;
        conduit_language::validate_language_coverage(&coverage)
            .map_err(|error| format!("retained Language coverage: {error:?}"))?;
        if coverage.evidence() != &format!("espeak/provider/{}", self.provider_sha256) {
            return Err(
                "retained Language coverage differs from selected provider identity".into(),
            );
        }
        Ok(coverage)
    }

    fn validate_inputs(&self) -> Result<(), String> {
        if (!self.artifact_only && self.card_id.is_empty())
            || (self.artifact_only
                && (!self.card_id.is_empty()
                    || self.device != 0
                    || !self.speaker_base_identity.is_empty()))
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

    /// Select the voice, optional speaker, and finite create-new WAV artifact pool
    /// before this Boot is advertised. Each Play gets its own exact name.
    /// Discovery does not open a PCM handle; the Back rechecks the device at Play.
    pub(super) fn attach_to_fresh_host_with_artifact(
        &self,
        host: &mut StdHost,
        artifact_root: &Path,
    ) -> Result<Option<AttachedEquipment>, String> {
        let offer = host.advertisement();
        let artifact = WavArtifactSelection::per_play_root(
            artifact_root,
            offer.boot_id.clone(),
            offer.offer_generation,
        )?;
        self.validate()?;
        let observation = if self.artifact_only {
            None
        } else {
            let observation = observe_speaker(&self.card_id, self.device)?;
            if observation.base_identity != self.speaker_base_identity {
                return Err(
                    "configured speaker observation identity changed; reselect equipment".into(),
                );
            }
            Some(observation)
        };
        let offered = host.advertisement().clone();
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
        let discovery = discovery
            .declare_language_coverage(self.coverage()?)
            .map_err(|error| format!("configured eSpeak Language coverage: {error:?}"))?;
        let adapter = discovery
            .initialize(
                offered.host_id.clone(),
                offered.boot_id.clone(),
                offered.offer_generation,
                "grant/installed-selected-speech".into(),
                Duration::from_secs(30),
            )
            .map_err(|error| format!("initialize configured eSpeak provider: {error:?}"))?;
        let realization_properties = adapter.offer().realization_properties;
        if self.artifact_only {
            host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
            return Ok(None);
        }
        let observation = observation.ok_or("selected speaker observation is missing")?;
        let playback = HostedPlaybackSelection::from_observation(
            observation,
            offered.boot_id.clone(),
            offered.offer_generation,
        )
        .with_bounded_speech_queue();
        host.attach_selected_playback(playback.clone())?;
        host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
        Ok(Some(AttachedEquipment {
            playback,
            authorization: ExplicitPlaybackAuthorization::new(&format!(
                "grant/installed-selected-speech/{}",
                offered.boot_id.as_str()
            ))?,
            provider_sha256,
            realization_properties,
            #[cfg(test)]
            before_play: None,
        }))
    }
}

pub(super) fn change(options: InstalledSpeechOptions) -> Result<Change, String> {
    if (options.selected_speech && options.selected_artifact_speech)
        || (options.without_selected_speech
            && (options.selected_speech || options.selected_artifact_speech))
        || (options.selected_artifact_speech
            && (options.speaker_card.is_some() || options.speaker_device.is_some()))
    {
        return Err("select one explicit speech output mode".into());
    }
    if options.without_selected_speech {
        return Ok(Change::Remove);
    }
    if !options.selected_speech && !options.selected_artifact_speech {
        return Ok(Change::Preserve);
    }
    let coverage = conduit_std_host::hosted_speech_synthesis::read_language_coverage(
        &options
            .speech_language_coverage
            .ok_or("selected speech needs Language coverage")?,
    )
    .map_err(|error| error.to_string())?;
    let artifact_only = options.selected_artifact_speech;
    let selection = Selection {
        artifact_only,
        card_id: if artifact_only {
            String::new()
        } else {
            options
                .speaker_card
                .ok_or("selected speech needs a speaker card")?
        },
        device: if artifact_only {
            0
        } else {
            options
                .speaker_device
                .ok_or("selected speech needs a speaker device")?
        },
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
        language_coverage: Some(
            coverage
                .clone()
                .encode()
                .map_err(|error| format!("encode selected Language coverage: {error:?}"))?,
        ),
    };
    selection.validate_inputs()?;
    let speaker_identity = if artifact_only {
        String::new()
    } else {
        observe_speaker(&selection.card_id, selection.device)?.base_identity
    };
    let discovery = EspeakDiscovery::inspect(
        &selection.executable,
        &selection.data_root,
        &selection.voice,
        &selection.engine_dependencies,
    )
    .map_err(|error| format!("review selected eSpeak provider: {error:?}"))?;
    let discovery = discovery
        .declare_language_coverage(coverage)
        .map_err(|error| format!("review selected eSpeak Language coverage: {error:?}"))?;
    Ok(Change::Replace(selection.reviewed_with(
        speaker_identity,
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
pub(super) fn fixture_retained_selection() -> Selection {
    Selection {
        artifact_only: false,
        card_id: "missing-card".into(),
        device: 0,
        speaker_base_identity: "missing-card-identity".into(),
        executable: "/missing/espeak-ng".into(),
        data_root: "/missing/espeak-ng-data".into(),
        voice: "en-us".into(),
        engine_dependencies: vec!["/missing/libespeak-ng.so".into()],
        provider_sha256: "a".repeat(64),
        language_coverage: Some(fixture_language_coverage(&"a".repeat(64))),
    }
}

#[cfg(test)]
pub(super) fn fixture_language_coverage(provider_sha256: &str) -> Vec<u8> {
    use conduit_language::{LanguageCoverage, LanguageId};
    use conduit_plot::rust_binding::BoundedSequence;
    LanguageCoverage::new(
        format!("espeak/provider/{provider_sha256}"),
        BoundedSequence::try_from_iter([LanguageId::new("language/english".into()).unwrap()])
            .unwrap(),
        BoundedSequence::try_from_iter([]).unwrap(),
        "selection-test@1".into(),
        BoundedSequence::try_from_iter([]).unwrap(),
        false,
    )
    .unwrap()
    .encode()
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_selection_retains_provider_identity_and_no_device_authority() {
        for options in [
            InstalledSpeechOptions {
                selected_artifact_speech: true,
                selected_speech: true,
                ..Default::default()
            },
            InstalledSpeechOptions {
                selected_artifact_speech: true,
                without_selected_speech: true,
                ..Default::default()
            },
            InstalledSpeechOptions {
                selected_artifact_speech: true,
                speaker_device: Some(0),
                ..Default::default()
            },
        ] {
            assert!(
                matches!(change(options), Err(code) if code == "select one explicit speech output mode")
            );
        }
        let physical = fixture_retained_selection();
        let encoded = serde_json::to_value(&physical).unwrap();
        assert!(encoded.get("artifact_only").is_none());
        let legacy: Selection = serde_json::from_value(encoded).unwrap();
        assert!(!legacy.artifact_only);
        legacy.validate().unwrap();
        let mut artifact = physical;
        artifact.artifact_only = true;
        artifact.card_id.clear();
        artifact.device = 0;
        artifact.speaker_base_identity.clear();
        artifact.validate().unwrap();
        let roundtrip: Selection =
            serde_json::from_value(serde_json::to_value(&artifact).unwrap()).unwrap();
        assert!(roundtrip.artifact_only);
        roundtrip.validate().unwrap();
        artifact.card_id = "unadmitted-card".into();
        assert!(artifact.validate().is_err());
        artifact.card_id.clear();
        artifact.provider_sha256 = "b".repeat(64);
        assert!(artifact
            .validate()
            .unwrap_err()
            .contains("provider identity"));
    }

    #[test]
    fn reviewed_selection_pins_bare_provider_digest_and_speaker_identity() {
        let unreviewed = Selection {
            artifact_only: false,
            card_id: "card".into(),
            device: 0,
            speaker_base_identity: String::new(),
            executable: "/bin/espeak-ng".into(),
            data_root: "/data/espeak-ng-data".into(),
            voice: "en-us".into(),
            engine_dependencies: vec!["/lib/libespeak-ng.so.1".into()],
            provider_sha256: String::new(),
            language_coverage: Some(fixture_language_coverage(&"a".repeat(64))),
        };
        let reviewed = unreviewed
            .clone()
            .reviewed_with("physical-card".into(), "a".repeat(64))
            .unwrap();
        assert_eq!(reviewed.speaker_base_identity, "physical-card");
        assert_eq!(reviewed.provider_sha256, "a".repeat(64));
        let mut legacy = serde_json::to_value(&reviewed).unwrap();
        legacy.as_object_mut().unwrap().remove("language_coverage");
        let legacy: Selection = serde_json::from_value(legacy).unwrap();
        assert!(legacy.validate().unwrap_err().contains("reselect"));
        let mut missing = reviewed.clone();
        missing.language_coverage = None;
        assert!(missing.validate().unwrap_err().contains("reselect"));
        let mut stale = reviewed.clone();
        stale.provider_sha256 = "b".repeat(64);
        assert!(stale.validate().unwrap_err().contains("provider identity"));
        assert!(unreviewed
            .reviewed_with("physical-card".into(), format!("sha256:{}", "a".repeat(64)))
            .is_err());
    }
}
