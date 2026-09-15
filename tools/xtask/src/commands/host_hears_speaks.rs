//! Recorded question through ordinary House Plan/Play with retained audio evidence.

use super::host_whisper::{encode_recorded_pcm, read_bounded_pcm};
use crate::cli::GlobalOpts;
use crate::evidence::{
    EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
};
use clap::Args;
use conduit_ai::LocalModelKindProfile;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::hosted_local_model::OllamaDiscovery;
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::hosted_wav_artifact::WavArtifactSelection;
use conduit_std_host::{StdHostComposition, StdHostConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalProviderProfile {
    schema: String,
    host: String,
    whisper_executable: PathBuf,
    whisper_executable_sha256: String,
    whisper_model: PathBuf,
    whisper_model_sha256: String,
    pcm_s16le_16000_mono: PathBuf,
    ollama_model: String,
    ollama_model_content_identity: String,
    admitted_memory_mib: u32,
    piper_executable: PathBuf,
    piper_executable_sha256: String,
    piper_model: PathBuf,
    piper_model_sha256: String,
    piper_config: PathBuf,
    piper_config_sha256: String,
    piper_library_path: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(super) struct HearsSpeaksArgs {
    #[arg(long)]
    whisper_executable: PathBuf,
    #[arg(long)]
    whisper_model: PathBuf,
    #[arg(long)]
    pcm_s16le_16000_mono: PathBuf,
    #[arg(long, default_value_t = 2)]
    whisper_threads: u8,
    #[arg(long, default_value_t = 30)]
    whisper_timeout_seconds: u64,
    #[arg(long)]
    ollama_model: String,
    #[arg(long)]
    admitted_memory_mib: u32,
    #[arg(long)]
    piper_executable: PathBuf,
    #[arg(long)]
    piper_model: PathBuf,
    #[arg(long)]
    piper_config: PathBuf,
    #[arg(long)]
    piper_library_path: Option<PathBuf>,
    #[arg(long, default_value_t = 30)]
    piper_timeout_seconds: u64,
    #[arg(long, default_value = "target/journeys/hears-speaks")]
    output: PathBuf,
}

pub(super) fn run(
    request: HearsSpeaksArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate(&request)?;
    if opts.dry_run {
        if opts.json {
            println!("{{\"schema\":\"conduit.journey/hears-speaks@1\",\"dry_run\":true}}");
        } else if !opts.quiet {
            println!(
                "would retain one recorded question and synthesized WAV from one House Plan/Play"
            );
        }
        return Ok(());
    }
    let parent = request
        .output
        .parent()
        .ok_or("journey evidence output has no parent directory")?;
    fs::create_dir_all(parent)?;
    fs::create_dir(&request.output)
        .map_err(|error| format!("create new journey evidence directory: {error}"))?;
    let mut evidence_directory = NewEvidenceDirectory::new(request.output.clone());
    let raw = read_bounded_pcm(&request.pcm_s16le_16000_mono)?;
    let (clip, _, _) = encode_recorded_pcm(&raw)?;
    fs::write(request.output.join("input.pcm"), &raw)?;
    write_mono_wav(&request.output.join("input.wav"), &raw)?;
    let whisper_discovery =
        WhisperDiscovery::inspect(&request.whisper_executable, &request.whisper_model)?;
    let whisper_identity = serde_json::json!({
        "implementation": "whisper.cpp",
        "executable_sha256": whisper_discovery.executable_sha256,
        "model_sha256": whisper_discovery.model_sha256,
        "model_bytes": whisper_discovery.model_bytes,
    });
    let whisper = whisper_discovery.initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: request.whisper_threads,
        timeout: Duration::from_secs(request.whisper_timeout_seconds),
    })?;
    let local_model_discovery = OllamaDiscovery::discover(&request.ollama_model)?;
    let local_model_identity = serde_json::to_value(&local_model_discovery)?;
    let local_model = local_model_discovery.initialize(
        request.admitted_memory_mib,
        vec![LocalModelKindProfile::Generate],
    )?;
    let piper_discovery = PiperDiscovery::inspect(
        &request.piper_executable,
        &request.piper_model,
        &request.piper_config,
        request.piper_library_path,
    )?;
    let piper_identity = serde_json::json!({
        "implementation": "piper",
        "executable_sha256": piper_discovery.executable_sha256,
        "model_sha256": piper_discovery.model_sha256,
        "config_sha256": piper_discovery.config_sha256,
        "model_bytes": piper_discovery.model_bytes,
        "sample_rate_hz": piper_discovery.sample_rate_hz,
    });
    let speech = piper_discovery.initialize(PiperLimits {
        maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_frames: conduit_std_offers::PIPER_MAXIMUM_FRAMES,
        maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
        timeout: Duration::from_secs(request.piper_timeout_seconds),
    })?;
    let boot_id = BootId::from("boot/journey-hears-speaks");
    let receipt = conduit_std_host::recorded_house_proof::run_recorded_with_wav(
        StdHostConfig {
            host_id: HostId::from("host/journey-hears-speaks"),
            boot_id: boot_id.clone(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(local_model),
        whisper,
        clip,
        speech,
        WavArtifactSelection::new(
            request.output.join("output.wav"),
            boot_id,
            OfferGeneration(1),
        )?,
    )?;
    fs::write(
        request.output.join("recognition.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "conduit.journey/hears-speaks-recognition@1",
            "text": receipt.recognized_text,
            "sha256": receipt.house.recognized_text_sha256,
            "bytes": receipt.house.recognized_text_bytes,
        }))?,
    )?;
    fs::write(
        request.output.join("response.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "conduit.journey/hears-speaks-response@1",
            "text": receipt.response_text,
            "sha256": receipt.house.response_sha256,
            "bytes": receipt.house.response_bytes,
        }))?,
    )?;
    let mut retained_receipt = serde_json::to_value(&receipt)?;
    retained_receipt
        .as_object_mut()
        .ok_or("journey receipt did not serialize as an object")?
        .insert(
            "providers".into(),
            serde_json::json!({
                "schema": "conduit.journey/hears-speaks-providers@1",
                "whisper": whisper_identity,
                "local_model": local_model_identity,
                "piper": piper_identity,
            }),
        );
    fs::write(
        request.output.join("receipt.json"),
        serde_json::to_vec_pretty(&retained_receipt)?,
    )?;
    write_manifest(
        &request.output,
        &receipt.house.plan_id,
        &receipt.house.play_id,
    )?;
    evidence_directory.retain();
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!(
            "HEARS/SPEAKS JOURNEY COMPLETE: plan={} play={} evidence={}",
            receipt.house.plan_id,
            receipt.house.play_id,
            request.output.display()
        );
    }
    Ok(())
}

pub(super) fn run_local(
    profile_path: &Path,
    output: PathBuf,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile: LocalProviderProfile = serde_json::from_slice(&fs::read(profile_path)?)?;
    if profile.schema != "conduit.journey/hears-speaks-local-provider@1" {
        return Err("unsupported Hears and Speaks local-provider profile".into());
    }
    let hostname = String::from_utf8(Command::new("hostname").output()?.stdout)?;
    let hostname = hostname.trim().split('.').next().unwrap_or("");
    if !matches!(hostname, "forebrain" | "victus") || hostname != profile.host {
        return Err(format!(
            "local provider profile is for '{}' but this admitted host is '{}'",
            profile.host, hostname
        )
        .into());
    }
    let whisper = WhisperDiscovery::inspect(&profile.whisper_executable, &profile.whisper_model)?;
    require_digest(
        "Whisper executable",
        &whisper.executable_sha256,
        &profile.whisper_executable_sha256,
    )?;
    require_digest(
        "Whisper model",
        &whisper.model_sha256,
        &profile.whisper_model_sha256,
    )?;
    let local_model = OllamaDiscovery::discover(&profile.ollama_model)?;
    require_digest(
        "Ollama model",
        &local_model.model_content_identity,
        &profile.ollama_model_content_identity,
    )?;
    let piper = PiperDiscovery::inspect(
        &profile.piper_executable,
        &profile.piper_model,
        &profile.piper_config,
        profile.piper_library_path.clone(),
    )?;
    require_digest(
        "Piper executable",
        &piper.executable_sha256,
        &profile.piper_executable_sha256,
    )?;
    require_digest(
        "Piper model",
        &piper.model_sha256,
        &profile.piper_model_sha256,
    )?;
    require_digest(
        "Piper config",
        &piper.config_sha256,
        &profile.piper_config_sha256,
    )?;
    run(
        HearsSpeaksArgs {
            whisper_executable: profile.whisper_executable,
            whisper_model: profile.whisper_model,
            pcm_s16le_16000_mono: profile.pcm_s16le_16000_mono,
            whisper_threads: 2,
            whisper_timeout_seconds: 60,
            ollama_model: profile.ollama_model,
            admitted_memory_mib: profile.admitted_memory_mib,
            piper_executable: profile.piper_executable,
            piper_model: profile.piper_model,
            piper_config: profile.piper_config,
            piper_library_path: profile.piper_library_path,
            piper_timeout_seconds: 60,
            output,
        },
        opts,
    )
}

fn require_digest(label: &str, actual: &str, expected: &str) -> Result<(), String> {
    let actual = actual.strip_prefix("sha256:").unwrap_or(actual);
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{label} profile identity is not one SHA-256 digest"
        ));
    }
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "{label} differs from the host-local pinned identity"
        ));
    }
    Ok(())
}

struct NewEvidenceDirectory {
    path: PathBuf,
    retain: bool,
}

impl NewEvidenceDirectory {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            retain: false,
        }
    }

    fn retain(&mut self) {
        self.retain = true;
    }
}

impl Drop for NewEvidenceDirectory {
    fn drop(&mut self) {
        if !self.retain {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn validate(request: &HearsSpeaksArgs) -> Result<(), String> {
    if !(1..=32).contains(&request.whisper_threads)
        || !(1..=120).contains(&request.whisper_timeout_seconds)
        || !(1..=120).contains(&request.piper_timeout_seconds)
    {
        return Err("journey provider bounds are outside their reviewed ranges".into());
    }
    Ok(())
}

fn write_manifest(root: &Path, plan_id: &str, play_id: &str) -> Result<(), String> {
    let workspace = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut manifest =
        EvidenceManifest::new(root, &workspace, "journey-hears-speaks", "journey-gallery")?;
    for (id, kind, path, media_type) in [
        (
            "hears-speaks.input-pcm",
            EvidenceKind::Audio,
            "input.pcm",
            "audio/L16; rate=16000; channels=1",
        ),
        (
            "hears-speaks.input-wav",
            EvidenceKind::Audio,
            "input.wav",
            "audio/wav",
        ),
        (
            "hears-speaks.recognition",
            EvidenceKind::MachineReadableManifest,
            "recognition.json",
            "application/json",
        ),
        (
            "hears-speaks.response",
            EvidenceKind::MachineReadableManifest,
            "response.json",
            "application/json",
        ),
        (
            "hears-speaks.output-wav",
            EvidenceKind::Audio,
            "output.wav",
            "audio/wav",
        ),
        (
            "hears-speaks.receipt",
            EvidenceKind::MachineReadableManifest,
            "receipt.json",
            "application/json",
        ),
    ] {
        manifest.declare(EvidenceOutput {
            id: id.into(),
            kind,
            path: path.into(),
            media_type: media_type.into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "hears-speaks.recorded-addressed-house@1".into(),
                plan_id: Some(plan_id.into()),
                active_play_id: Some(play_id.into()),
                asserted_semantic_disposition: Some("completed".into()),
                proof_class: Some("hosted-recorded-audio-plan-play".into()),
                ..EvidenceProvenance::default()
            },
        })?;
    }
    manifest.finish(EvidenceResult::Complete)
}

fn write_mono_wav(path: &Path, pcm: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let data = u32::try_from(pcm.len())?;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(
        &(36_u32.checked_add(data).ok_or("WAV extent overflowed")?).to_le_bytes(),
    );
    wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    wav.extend_from_slice(&16_000_u32.to_le_bytes());
    wav.extend_from_slice(&32_000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.extend_from_slice(pcm);
    fs::write(path, wav)?;
    Ok(())
}
