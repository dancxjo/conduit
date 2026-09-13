use super::host_whisper::{encode_recorded_pcm, read_bounded_pcm};
use crate::cli::GlobalOpts;
use conduit_ai::LocalModelKindProfile;
use conduit_std_host::hosted_local_model::OllamaDiscovery;
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

pub(super) struct RecordedHouseRequest {
    pub whisper_executable: PathBuf,
    pub whisper_model: PathBuf,
    pub pcm_s16le_16000_mono: PathBuf,
    pub whisper_threads: u8,
    pub whisper_timeout_seconds: u64,
    pub ollama_model: String,
    pub admitted_memory_mib: u32,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    receipt: Option<conduit_std_host::recorded_house_proof::RecordedHouseProofReceipt>,
}

pub(super) fn prove(
    request: RecordedHouseRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.whisper_timeout_seconds == 0 || request.whisper_timeout_seconds > 120 {
        return Err("Whisper timeout must be between 1 and 120 seconds".into());
    }
    if !(1..=32).contains(&request.whisper_threads) {
        return Err("Whisper threads must be between 1 and 32".into());
    }
    if opts.dry_run {
        return emit(
            Report {
                schema: "conduit.tools/xtask/recorded-house-plan-play-proof@1",
                proof_class: "recorded-speech-addressed-local-model-plan-play",
                dry_run: true,
                effects_performed: false,
                receipt: None,
            },
            opts,
        );
    }
    let raw = read_bounded_pcm(&request.pcm_s16le_16000_mono)?;
    let (clip, _, _) = encode_recorded_pcm(&raw)?;
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
    let receipt = conduit_std_host::recorded_house_proof::run(
        StdHostConfig {
            host_id: conduit_core::HostId::from("host/recorded-house-proof"),
            boot_id: conduit_core::BootId::from("boot/recorded-house-proof"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(local_model),
        whisper,
        clip,
    )?;
    emit(
        Report {
            schema: "conduit.tools/xtask/recorded-house-plan-play-proof@1",
            proof_class: "recorded-speech-addressed-local-model-plan-play",
            dry_run: false,
            effects_performed: true,
            receipt: Some(receipt),
        },
        opts,
    )
}

fn emit(report: Report, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        if let Some(receipt) = report.receipt {
            println!(
                "RECORDED HOUSE PLAN/PLAY PROVED: plan={} play={} response-bytes={}",
                receipt.plan_id, receipt.play_id, receipt.response_bytes
            );
        } else {
            println!("would run recorded speech through one address-gated local House Plan");
        }
    }
    Ok(())
}
