use crate::cli::GlobalOpts;
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
    pub maximum_frames: u32,
    pub maximum_blocks: u16,
    pub timeout_seconds: u64,
}

#[derive(Serialize)]
struct PiperProofReport<'a> {
    schema: &'static str,
    proof_class: &'static str,
    dry_run: bool,
    effects_performed: bool,
    executable_sha256: Option<&'a str>,
    model_sha256: Option<&'a str>,
    config_sha256: Option<&'a str>,
    model_bytes: Option<u64>,
    sample_rate_hz: Option<u32>,
    text_sha256: Option<&'a str>,
    pcm_sha256: Option<&'a str>,
    frames: Option<u32>,
    blocks: Option<u16>,
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
        let report = PiperProofReport {
            schema: "conduit.tools/xtask/piper-provider-proof@1",
            proof_class: "provider-not-plan-play",
            dry_run: true,
            effects_performed: false,
            executable_sha256: None,
            model_sha256: None,
            config_sha256: None,
            model_bytes: None,
            sample_rate_hz: None,
            text_sha256: None,
            pcm_sha256: None,
            frames: None,
            blocks: None,
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
        executable_sha256: Some(&executable_sha256),
        model_sha256: Some(&model_sha256),
        config_sha256: Some(&config_sha256),
        model_bytes: Some(model_bytes),
        sample_rate_hz: Some(sample_rate_hz),
        text_sha256: Some(&receipt.text_sha256),
        pcm_sha256: Some(&receipt.pcm_sha256),
        frames: Some(receipt.frames),
        blocks: Some(receipt.blocks),
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
