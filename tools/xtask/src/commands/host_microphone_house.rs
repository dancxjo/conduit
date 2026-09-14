use crate::cli::GlobalOpts;
use clap::Args;
use conduit_ai::LocalModelKindProfile;
use conduit_std_host::hosted_local_model::OllamaDiscovery;
use conduit_std_host::hosted_microphone::{
    AlsaMicrophoneDiscovery, MicrophoneLimits, MAXIMUM_CAPTURE_MILLISECONDS,
};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Debug)]
pub(super) struct MicrophoneHouseArgs {
    #[arg(long)]
    arecord_executable: PathBuf,
    #[arg(long)]
    card_id: String,
    #[arg(long)]
    device: u16,
    #[arg(long, default_value_t = 3_000)]
    capture_milliseconds: u32,
    #[arg(long, default_value_t = 10)]
    capture_timeout_seconds: u64,
    #[arg(long)]
    whisper_executable: PathBuf,
    #[arg(long)]
    whisper_model: PathBuf,
    #[arg(long, default_value_t = 2)]
    whisper_threads: u8,
    #[arg(long, default_value_t = 30)]
    whisper_timeout_seconds: u64,
    #[arg(long)]
    ollama_model: String,
    #[arg(long)]
    admitted_memory_mib: u32,
    #[arg(long)]
    authorize_capture: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    capture_authorized: bool,
    receipt: Option<conduit_std_host::recorded_house_proof::MicrophoneHouseProofReceipt>,
}

pub(super) fn prove(
    request: MicrophoneHouseArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate(&request)?;
    if opts.dry_run {
        return emit(
            Report {
                schema: "conduit.tools/xtask/microphone-house-plan-play-proof@1",
                proof_class: "explicit-microphone-addressed-local-model-plan-play",
                dry_run: true,
                effects_performed: false,
                capture_authorized: request.authorize_capture,
                receipt: None,
            },
            opts,
        );
    }
    if !request.authorize_capture {
        return Err("live microphone House proof requires --authorize-capture".into());
    }
    let discovery = AlsaMicrophoneDiscovery::inspect(&request.arecord_executable)?;
    let matches = discovery
        .observations
        .iter()
        .filter(|candidate| {
            candidate.card_id == request.card_id && candidate.device == request.device
        })
        .cloned()
        .collect::<Vec<_>>();
    let [selected] = matches.as_slice() else {
        return Err(format!(
            "selected microphone card-id/device matched {} current endpoints",
            matches.len()
        )
        .into());
    };
    let microphone = discovery.initialize(
        selected,
        MicrophoneLimits {
            capture_milliseconds: request.capture_milliseconds,
            timeout: Duration::from_secs(request.capture_timeout_seconds),
        },
    )?;
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
    let receipt = conduit_std_host::recorded_house_proof::run_microphone(
        StdHostConfig {
            host_id: conduit_core::HostId::from("host/microphone-house-proof"),
            boot_id: conduit_core::BootId::from("boot/microphone-house-proof"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(local_model),
        whisper,
        microphone,
    )?;
    emit(
        Report {
            schema: "conduit.tools/xtask/microphone-house-plan-play-proof@1",
            proof_class: "explicit-microphone-addressed-local-model-plan-play",
            dry_run: false,
            effects_performed: true,
            capture_authorized: true,
            receipt: Some(receipt),
        },
        opts,
    )
}

fn validate(request: &MicrophoneHouseArgs) -> Result<(), Box<dyn std::error::Error>> {
    if request.card_id.is_empty() || request.card_id.len() > 64 {
        return Err("microphone card id must contain 1 to 64 bytes".into());
    }
    if request.capture_milliseconds == 0
        || request.capture_milliseconds > MAXIMUM_CAPTURE_MILLISECONDS
    {
        return Err("capture duration must be between 1 and 6000 milliseconds".into());
    }
    if request.capture_timeout_seconds == 0 || request.capture_timeout_seconds > 120 {
        return Err("capture timeout must be between 1 and 120 seconds".into());
    }
    if !(1..=32).contains(&request.whisper_threads) {
        return Err("Whisper threads must be between 1 and 32".into());
    }
    if request.whisper_timeout_seconds == 0 || request.whisper_timeout_seconds > 120 {
        return Err("Whisper timeout must be between 1 and 120 seconds".into());
    }
    Ok(())
}

fn emit(report: Report, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        if let Some(receipt) = report.receipt {
            println!(
                "MICROPHONE HOUSE PLAN/PLAY PROVED: plan={} play={} captured-bytes={} response-bytes={}",
                receipt.house.plan_id,
                receipt.house.play_id,
                receipt.raw_pcm_bytes,
                receipt.house.response_bytes
            );
        } else {
            println!("would capture one authorized clip through one address-gated House Plan");
        }
    }
    Ok(())
}
