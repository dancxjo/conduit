//! Explicit durable configuration of already-local Voice Host providers.

use conduit_ai::LocalModelKindProfile;
use conduit_std_host::hosted_local_model::OllamaDiscovery;
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::VoiceHostProviders;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SCHEMA: &str = "conduit.install/durable-voice-providers@1";
const FILE: &str = "voice-providers.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DurableVoiceProviderConfig {
    schema: String,
    whisper_executable: PathBuf,
    whisper_model: PathBuf,
    whisper_threads: u8,
    whisper_timeout_seconds: u64,
    ollama_model: String,
    admitted_memory_mib: u32,
    piper_executable: PathBuf,
    piper_model: PathBuf,
    piper_config: PathBuf,
    piper_library_path: Option<PathBuf>,
    piper_timeout_seconds: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn configure(
    state_dir: &Path,
    whisper_executable: PathBuf,
    whisper_model: PathBuf,
    whisper_threads: u8,
    whisper_timeout_seconds: u64,
    ollama_model: String,
    admitted_memory_mib: u32,
    piper_executable: PathBuf,
    piper_model: PathBuf,
    piper_config: PathBuf,
    piper_library_path: Option<PathBuf>,
    piper_timeout_seconds: u64,
    authorize_local_voice: bool,
) -> Result<(), String> {
    if !authorize_local_voice {
        return Err("configuring local Voice providers requires --authorize-local-voice".into());
    }
    if !state_dir.join("installation.json").is_file() {
        return Err("durable Host must be installed before configuring Voice providers".into());
    }
    let config = DurableVoiceProviderConfig {
        schema: SCHEMA.into(),
        whisper_executable,
        whisper_model,
        whisper_threads,
        whisper_timeout_seconds,
        ollama_model,
        admitted_memory_mib,
        piper_executable,
        piper_model,
        piper_config,
        piper_library_path,
        piper_timeout_seconds,
    };
    validate(&config)?;
    // Prove the selected providers are genuinely local and initialized before
    // publishing configuration that the durable service will later advertise.
    drop(initialize(&config)?);
    super::write_json_atomic(&state_dir.join(FILE), &config)?;
    println!(
        "configured truthful local Voice providers for durable Host state {}",
        state_dir.display()
    );
    println!("restart the durable Host service to publish the new provider offers");
    Ok(())
}

pub(super) fn load(state_dir: &Path) -> Result<Option<VoiceHostProviders>, String> {
    let path = state_dir.join(FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = super::bounded_read(&path, 64 * 1024)?;
    let config: DurableVoiceProviderConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("durable Voice provider configuration: {error}"))?;
    validate(&config)?;
    initialize(&config).map(Some)
}

fn validate(config: &DurableVoiceProviderConfig) -> Result<(), String> {
    if config.schema != SCHEMA {
        return Err("durable Voice provider configuration has an unsupported schema".into());
    }
    if !(1..=32).contains(&config.whisper_threads)
        || !(1..=120).contains(&config.whisper_timeout_seconds)
        || !(1..=120).contains(&config.piper_timeout_seconds)
        || config.ollama_model.is_empty()
        || config.ollama_model.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES
        || config.admitted_memory_mib == 0
    {
        return Err("durable Voice provider configuration violates its finite bounds".into());
    }
    Ok(())
}

fn initialize(config: &DurableVoiceProviderConfig) -> Result<VoiceHostProviders, String> {
    let recognition = WhisperDiscovery::inspect(
        &config.whisper_executable,
        &config.whisper_model,
    )
    .map_err(|error| format!("discover configured Whisper provider: {error:?}"))?
    .initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: config.whisper_threads,
        timeout: Duration::from_secs(config.whisper_timeout_seconds),
    })
    .map_err(|error| format!("initialize configured Whisper provider: {error:?}"))?;

    let model = OllamaDiscovery::discover(&config.ollama_model)
        .map_err(|error| format!("discover configured Ollama model: {error}"))?
        .initialize(
            config.admitted_memory_mib,
            vec![LocalModelKindProfile::Generate],
        )
        .map_err(|error| format!("initialize configured Ollama model: {error}"))?;

    let synthesis = PiperDiscovery::inspect(
        &config.piper_executable,
        &config.piper_model,
        &config.piper_config,
        config.piper_library_path.clone(),
    )
    .map_err(|error| format!("discover configured Piper provider: {error:?}"))?
    .initialize(PiperLimits {
        maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_frames: conduit_std_offers::PIPER_MAXIMUM_FRAMES,
        maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
        timeout: Duration::from_secs(config.piper_timeout_seconds),
    })
    .map_err(|error| format!("initialize configured Piper provider: {error:?}"))?;

    Ok(VoiceHostProviders {
        recognition,
        model: Box::new(model),
        synthesis,
    })
}
