use crate::cli::GlobalOpts;
use conduit_core::{BootId, OfferGeneration};
use conduit_std_host::hosted_audio::{discover_alsa_playback, HostedPlaybackSelection};
use conduit_std_host::hosted_speech::{PiperDiscovery, PiperLimits};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

pub(super) struct PiperProofRequest {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
    pub library_path: Option<PathBuf>,
    pub text: String,
    pub plan_play: bool,
    pub playback_card_id: Option<String>,
    pub playback_device: Option<u16>,
    pub authorize_output: bool,
    pub maximum_frames: u32,
    pub maximum_blocks: u16,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
struct PiperProofReport {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    plan_id: Option<String>,
    active_play_id: Option<String>,
    implementation_id: Option<String>,
    executable_sha256: Option<String>,
    model_sha256: Option<String>,
    config_sha256: Option<String>,
    model_bytes: Option<u64>,
    sample_rate_hz: Option<u32>,
    text_sha256: Option<String>,
    pcm_sha256: Option<String>,
    frames: Option<u32>,
    blocks: Option<u16>,
    conversion_implementation_id: Option<String>,
    output_sample_rate_hz: Option<u32>,
    output_channels: Option<u8>,
    output_frames: Option<u64>,
    output_blocks: Option<u16>,
    playback_resource_pool_id: Option<String>,
    playback_alsa_target: Option<String>,
    playback_blocks: Option<u32>,
    playback_frames: Option<u64>,
    diagnostic_bytes: Option<u16>,
}

pub(super) fn prove(
    request: PiperProofRequest,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.timeout_seconds == 0 || request.timeout_seconds > 120 {
        return Err("Piper proof timeout must be between 1 and 120 seconds".into());
    }
    if opts.dry_run {
        let playback = request.playback_card_id.is_some();
        let report = PiperProofReport {
            schema: if playback {
                "conduit.tools/xtask/piper-playback-plan-play-proof@1"
            } else if request.plan_play {
                "conduit.tools/xtask/piper-plan-play-proof@2"
            } else {
                "conduit.tools/xtask/piper-provider-proof@1"
            },
            proof_class: if playback {
                "ordinary-plan-play-real-playback"
            } else if request.plan_play {
                "ordinary-plan-play"
            } else {
                "provider-not-plan-play"
            },
            dry_run: true,
            effects_performed: false,
            plan_id: None,
            active_play_id: None,
            implementation_id: None,
            executable_sha256: None,
            model_sha256: None,
            config_sha256: None,
            model_bytes: None,
            sample_rate_hz: None,
            text_sha256: None,
            pcm_sha256: None,
            frames: None,
            blocks: None,
            conversion_implementation_id: None,
            output_sample_rate_hz: None,
            output_channels: None,
            output_frames: None,
            output_blocks: None,
            playback_resource_pool_id: None,
            playback_alsa_target: None,
            playback_blocks: None,
            playback_frames: None,
            diagnostic_bytes: None,
        };
        if opts.json {
            println!("{}", serde_json::to_string(&report)?);
        } else if !opts.quiet {
            println!("would inspect and exercise one explicit bounded Piper provider");
        }
        return Ok(());
    }
    let discovery = PiperDiscovery::inspect(
        &request.executable,
        &request.model,
        &request.config,
        request.library_path,
    )?;
    let executable_sha256 = discovery.executable_sha256.clone();
    let model_sha256 = discovery.model_sha256.clone();
    let config_sha256 = discovery.config_sha256.clone();
    let model_bytes = discovery.model_bytes;
    let sample_rate_hz = discovery.sample_rate_hz;
    let limits = PiperLimits {
        maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_frames: request.maximum_frames,
        maximum_blocks: request.maximum_blocks,
        timeout: Duration::from_secs(request.timeout_seconds),
    };
    if request.plan_play {
        if let (Some(card_id), Some(device)) =
            (request.playback_card_id.as_deref(), request.playback_device)
        {
            if !request.authorize_output {
                return Err("Piper playback proof requires --authorize-output".into());
            }
            let matches = discover_alsa_playback()?
                .into_iter()
                .filter(|item| item.card_id == card_id && item.device == device)
                .collect::<Vec<_>>();
            let observation = match matches.as_slice() {
                [observation] => observation.clone(),
                [] => return Err(format!("selected ALSA playback resource card-id={card_id} device={device} is not freshly observed").into()),
                _ => return Err("selected ALSA playback identity is ambiguous".into()),
            };
            let boot_id = BootId::from(format!(
                "boot-piper-playback-proof-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
            ));
            let selection =
                HostedPlaybackSelection::from_observation(observation, boot_id, OfferGeneration(1));
            let run = conduit_std_host::piper_plan_play_proof::run_with_playback(
                discovery,
                limits,
                &request.text,
                selection,
                "grant/xtask-explicit-piper-playback",
            )?;
            let receipt = run
                .run
                .speech_synthesis
                .first()
                .ok_or("Piper playback proof omitted its synthesis receipt")?;
            let playback = run
                .run
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.playback.first())
                .ok_or("Piper playback proof omitted its playback receipt")?;
            let report = PiperProofReport {
                schema: "conduit.tools/xtask/piper-playback-plan-play-proof@1",
                proof_class: "ordinary-plan-play-real-playback",
                dry_run: false,
                effects_performed: true,
                plan_id: Some(receipt.plan_id.as_str().to_string()),
                active_play_id: Some(receipt.active_play_id.as_str().to_string()),
                implementation_id: Some(receipt.implementation_id.as_str().to_string()),
                executable_sha256: Some(receipt.executable_sha256.clone()),
                model_sha256: Some(receipt.model_sha256.clone()),
                config_sha256: Some(receipt.config_sha256.clone()),
                model_bytes: Some(model_bytes),
                sample_rate_hz: Some(sample_rate_hz),
                text_sha256: Some(receipt.text_sha256.clone()),
                pcm_sha256: Some(receipt.pcm_sha256.clone()),
                frames: Some(receipt.frames),
                blocks: Some(receipt.blocks),
                conversion_implementation_id: Some(
                    run.conversion_implementation_id.as_str().to_string(),
                ),
                output_sample_rate_hz: Some(48_000),
                output_channels: Some(2),
                output_frames: Some(run.output_frames),
                output_blocks: Some(receipt.blocks),
                playback_resource_pool_id: Some(run.resource_pool_id.as_str().to_string()),
                playback_alsa_target: Some(run.alsa_target),
                playback_blocks: Some(playback.metrics.blocks_committed),
                playback_frames: Some(playback.metrics.frames_committed),
                diagnostic_bytes: None,
            };
            if opts.json {
                println!("{}", serde_json::to_string(&report)?);
            } else if !opts.quiet {
                println!(
                    "PIPER PLAYBACK PLAN/PLAY PROVED: plan={} play={} target={} frames={}",
                    receipt.plan_id.as_str(),
                    receipt.active_play_id.as_str(),
                    report.playback_alsa_target.as_deref().unwrap_or(""),
                    playback.metrics.frames_committed
                );
            }
            return Ok(());
        }
        let run = conduit_std_host::piper_plan_play_proof::run(discovery, limits, &request.text)?;
        let receipt = run
            .run
            .speech_synthesis
            .first()
            .ok_or("Piper Plan/Play proof omitted its synthesis receipt")?;
        let report = PiperProofReport {
            schema: "conduit.tools/xtask/piper-plan-play-proof@2",
            proof_class: "ordinary-plan-play",
            dry_run: false,
            effects_performed: true,
            plan_id: Some(receipt.plan_id.as_str().to_string()),
            active_play_id: Some(receipt.active_play_id.as_str().to_string()),
            implementation_id: Some(receipt.implementation_id.as_str().to_string()),
            executable_sha256: Some(receipt.executable_sha256.clone()),
            model_sha256: Some(receipt.model_sha256.clone()),
            config_sha256: Some(receipt.config_sha256.clone()),
            model_bytes: Some(model_bytes),
            sample_rate_hz: Some(sample_rate_hz),
            text_sha256: Some(receipt.text_sha256.clone()),
            pcm_sha256: Some(receipt.pcm_sha256.clone()),
            frames: Some(receipt.frames),
            blocks: Some(receipt.blocks),
            conversion_implementation_id: Some(
                run.conversion_implementation_id.as_str().to_string(),
            ),
            output_sample_rate_hz: Some(48_000),
            output_channels: Some(2),
            output_frames: Some(run.output_frames),
            output_blocks: Some(receipt.blocks),
            playback_resource_pool_id: None,
            playback_alsa_target: None,
            playback_blocks: None,
            playback_frames: None,
            diagnostic_bytes: None,
        };
        if opts.json {
            println!("{}", serde_json::to_string(&report)?);
        } else if !opts.quiet {
            println!(
                "PIPER PLAN/PLAY PROVED: plan={} play={} model={} source_frames={} blocks={} pcm={} conversion={} output=stereo-s16le/48000Hz output_frames={}",
                receipt.plan_id.as_str(),
                receipt.active_play_id.as_str(),
                receipt.model_sha256,
                receipt.frames,
                receipt.blocks,
                receipt.pcm_sha256,
                run.conversion_implementation_id.as_str(),
                run.output_frames
            );
        }
        return Ok(());
    }
    let mut adapter = discovery.initialize(PiperLimits {
        maximum_text_bytes: conduit_tongues::MAXIMUM_TEXT_BYTES,
        maximum_frames: request.maximum_frames,
        maximum_blocks: request.maximum_blocks,
        timeout: Duration::from_secs(request.timeout_seconds),
    })?;
    let mut observed_blocks = 0_u16;
    let receipt = adapter.synthesize(
        &request.text,
        || false,
        |encoded| {
            conduit_audio::PcmFrameHeader::decode_frame(encoded).map_err(|_| ())?;
            observed_blocks = observed_blocks.checked_add(1).ok_or(())?;
            Ok(())
        },
    )?;
    if observed_blocks != receipt.blocks {
        return Err("Piper provider receipt did not match delivered block count".into());
    }
    let report = PiperProofReport {
        schema: "conduit.tools/xtask/piper-provider-proof@1",
        proof_class: "provider-not-plan-play",
        dry_run: false,
        effects_performed: true,
        plan_id: None,
        active_play_id: None,
        implementation_id: None,
        executable_sha256: Some(executable_sha256),
        model_sha256: Some(model_sha256.clone()),
        config_sha256: Some(config_sha256),
        model_bytes: Some(model_bytes),
        sample_rate_hz: Some(sample_rate_hz),
        text_sha256: Some(receipt.text_sha256.clone()),
        pcm_sha256: Some(receipt.pcm_sha256.clone()),
        frames: Some(receipt.frames),
        blocks: Some(receipt.blocks),
        conversion_implementation_id: None,
        output_sample_rate_hz: None,
        output_channels: None,
        output_frames: None,
        output_blocks: None,
        playback_resource_pool_id: None,
        playback_alsa_target: None,
        playback_blocks: None,
        playback_frames: None,
        diagnostic_bytes: Some(receipt.diagnostic_bytes),
    };
    if opts.json {
        println!("{}", serde_json::to_string(&report)?);
    } else if !opts.quiet {
        println!(
            "PIPER PROVIDER PROVED (not Plan/Play): model={} rate={}Hz frames={} blocks={} pcm={}",
            model_sha256, sample_rate_hz, receipt.frames, receipt.blocks, receipt.pcm_sha256
        );
    }
    Ok(())
}
