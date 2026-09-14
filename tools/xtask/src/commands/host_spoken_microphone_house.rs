use super::host_microphone_house::{self, MicrophoneHouseArgs};
use crate::cli::GlobalOpts;
use clap::Args;
use conduit_core::{BootId, OfferGeneration};
use conduit_std_host::hosted_audio::{discover_alsa_playback, HostedPlaybackSelection};
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use serde::Serialize;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Args, Debug)]
pub(super) struct SpokenMicrophoneHouseArgs {
    #[command(flatten)]
    microphone_house: MicrophoneHouseArgs,
    #[arg(long)]
    piper_executable: PathBuf,
    #[arg(long)]
    piper_model: PathBuf,
    #[arg(long)]
    piper_config: PathBuf,
    #[arg(long)]
    piper_library_path: Option<PathBuf>,
    #[arg(long, default_value_t = conduit_std_offers::PIPER_MAXIMUM_FRAMES)]
    piper_maximum_frames: u32,
    #[arg(long, default_value_t = conduit_std_offers::PIPER_MAXIMUM_BLOCKS)]
    piper_maximum_blocks: u16,
    #[arg(long, default_value_t = 30)]
    piper_timeout_seconds: u64,
    #[arg(long)]
    playback_card_id: String,
    #[arg(long)]
    playback_device: u16,
    #[arg(long)]
    authorize_output: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    capture_authorized: bool,
    output_authorized: bool,
    receipt: Option<conduit_std_host::recorded_house_proof::SpokenMicrophoneHouseProofReceipt>,
}

pub(super) fn prove(
    request: SpokenMicrophoneHouseArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    host_microphone_house::validate(&request.microphone_house)?;
    let piper_limits = PiperLimits {
        maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_frames: request.piper_maximum_frames,
        maximum_blocks: request.piper_maximum_blocks,
        timeout: Duration::from_secs(request.piper_timeout_seconds),
    }
    .validate()?;
    if request.playback_card_id.is_empty() || request.playback_card_id.len() > 64 {
        return Err("playback card id must contain 1 to 64 bytes".into());
    }
    if opts.dry_run {
        return emit(
            Report {
                schema: "conduit.tools/xtask/spoken-microphone-house-plan-play-proof@1",
                proof_class: "explicit-microphone-addressed-local-model-spoken-plan-play",
                dry_run: true,
                effects_performed: false,
                capture_authorized: request.microphone_house.capture_authorized(),
                output_authorized: request.authorize_output,
                receipt: None,
            },
            opts,
        );
    }
    if !request.microphone_house.capture_authorized() || !request.authorize_output {
        return Err(
            "live spoken microphone House proof requires --authorize-capture and --authorize-output"
                .into(),
        );
    }

    let initialized = host_microphone_house::initialize(&request.microphone_house)?;
    let speech = PiperDiscovery::inspect(
        &request.piper_executable,
        &request.piper_model,
        &request.piper_config,
        request.piper_library_path,
    )?
    .initialize(piper_limits)?;
    let matches = discover_alsa_playback()?
        .into_iter()
        .filter(|candidate| {
            candidate.card_id == request.playback_card_id
                && candidate.device == request.playback_device
        })
        .collect::<Vec<_>>();
    let [observation] = matches.as_slice() else {
        return Err(format!(
            "selected playback card-id/device matched {} current endpoints",
            matches.len()
        )
        .into());
    };
    let boot_id = BootId::from(format!(
        "boot/spoken-microphone-house-proof/{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let playback = HostedPlaybackSelection::from_observation(
        observation.clone(),
        boot_id.clone(),
        OfferGeneration(1),
    );
    let receipt = conduit_std_host::recorded_house_proof::run_spoken_microphone(
        StdHostConfig {
            host_id: conduit_core::HostId::from("host/spoken-microphone-house-proof"),
            boot_id,
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        initialized.local_model,
        initialized.whisper,
        initialized.microphone,
        speech,
        playback,
    )?;
    emit(
        Report {
            schema: "conduit.tools/xtask/spoken-microphone-house-plan-play-proof@1",
            proof_class: "explicit-microphone-addressed-local-model-spoken-plan-play",
            dry_run: false,
            effects_performed: true,
            capture_authorized: true,
            output_authorized: true,
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
                "SPOKEN MICROPHONE HOUSE PLAN/PLAY PROVED: plan={} play={} source-frames={} playback-frames={}",
                receipt.microphone_house.house.plan_id,
                receipt.microphone_house.house.play_id,
                receipt.source_pcm_frames,
                receipt.playback_frames_committed
            );
        } else {
            println!("would capture, recognize, address, generate, synthesize, convert, and play one bounded House response");
        }
    }
    Ok(())
}
