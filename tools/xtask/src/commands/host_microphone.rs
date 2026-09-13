use crate::cli::GlobalOpts;
use conduit_std_host::hosted_microphone::{
    AlsaMicrophoneDiscovery, MicrophoneLimits, MAXIMUM_CAPTURE_MILLISECONDS,
};
use conduit_std_host::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

pub(super) struct MicrophoneWhisperRequest {
    pub arecord_executable: PathBuf,
    pub card_id: String,
    pub device: u16,
    pub capture_milliseconds: u32,
    pub capture_timeout_seconds: u64,
    pub whisper_executable: PathBuf,
    pub whisper_model: PathBuf,
    pub whisper_threads: u8,
    pub whisper_timeout_seconds: u64,
    pub authorize_capture: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    capture_authorized: bool,
    receipt: Option<conduit_std_host::microphone_whisper_proof::MicrophoneWhisperProofReceipt>,
}

pub(super) fn prove(
    request: MicrophoneWhisperRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate(&request)?;
    if opts.dry_run {
        return emit(
            Report {
                schema: "conduit.tools/xtask/microphone-whisper-plan-play-proof@1",
                proof_class: "explicit-microphone-capture-whisper-plan-play",
                dry_run: true,
                effects_performed: false,
                capture_authorized: request.authorize_capture,
                receipt: None,
            },
            opts,
        );
    }
    if !request.authorize_capture {
        return Err("live microphone proof requires --authorize-capture".into());
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
    let receipt = conduit_std_host::microphone_whisper_proof::run(
        StdHostConfig {
            host_id: conduit_core::HostId::from("host/microphone-whisper-proof"),
            boot_id: conduit_core::BootId::from("boot/microphone-whisper-proof"),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        microphone,
        whisper,
    )?;
    emit(
        Report {
            schema: "conduit.tools/xtask/microphone-whisper-plan-play-proof@1",
            proof_class: "explicit-microphone-capture-whisper-plan-play",
            dry_run: false,
            effects_performed: true,
            capture_authorized: true,
            receipt: Some(receipt),
        },
        opts,
    )
}

fn validate(request: &MicrophoneWhisperRequest) -> Result<(), Box<dyn std::error::Error>> {
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
                "MICROPHONE WHISPER PLAN/PLAY PROVED: plan={} play={} captured-bytes={}",
                receipt.plan_id, receipt.play_id, receipt.raw_pcm_bytes
            );
        } else {
            println!("would capture one authorized bounded microphone clip and recognize it");
        }
    }
    Ok(())
}
