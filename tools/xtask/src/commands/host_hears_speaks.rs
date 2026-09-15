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
use std::time::Duration;

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
    let whisper = WhisperDiscovery::inspect(&request.whisper_executable, &request.whisper_model)?
        .initialize(WhisperLimits {
        maximum_audio_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        threads: request.whisper_threads,
        timeout: Duration::from_secs(request.whisper_timeout_seconds),
    })?;
    let local_model = OllamaDiscovery::discover(&request.ollama_model)?.initialize(
        request.admitted_memory_mib,
        vec![LocalModelKindProfile::Generate],
    )?;
    let speech = PiperDiscovery::inspect(
        &request.piper_executable,
        &request.piper_model,
        &request.piper_config,
        request.piper_library_path,
    )?
    .initialize(PiperLimits {
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
    fs::write(
        request.output.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt)?,
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
