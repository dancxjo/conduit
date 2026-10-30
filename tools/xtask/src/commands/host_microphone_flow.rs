//! Explicit finite Linux capture proof; no inference, routing or Body admission.
use crate::cli::GlobalOpts;
use conduit_std_host::hosted_microphone::{
    AlsaMicrophoneDiscovery, AlsaMicrophoneFormat, MicrophoneLimits,
};
use std::{path::PathBuf, time::Duration};
#[derive(clap::Args, Debug)]
pub(super) struct Request {
    #[arg(long)]
    arecord_executable: PathBuf,
    #[arg(long)]
    card_id: String,
    #[arg(long, default_value_t = 0)]
    device: u16,
    #[arg(long)]
    alsa_plug: bool,
    #[arg(long, default_value_t = 3000)]
    capture_milliseconds: u32,
    #[arg(long, default_value_t = 1024)]
    block_frames: u16,
    #[arg(long, default_value_t = 10)]
    timeout_seconds: u64,
    #[arg(long)]
    authorize_capture: bool,
}
pub(super) fn prove(request: Request, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if request.capture_milliseconds == 0
        || request.capture_milliseconds > 6000
        || !(1..=120).contains(&request.timeout_seconds)
    {
        return Err("capture must be 1..6000 milliseconds; timeout must be 1..120 seconds".into());
    }
    conduit_std_host::hosted_microphone::capture_flow::MicrophoneFlowLimits {
        total_frames: request.capture_milliseconds * 16,
        block_frames: request.block_frames,
    }
    .block_count()?;
    if opts.dry_run {
        if opts.json {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.tools/microphone-flow-proof@1","dry_run":true,"effects_performed":false})
            );
        } else if !opts.quiet {
            println!("would capture one finite microphone Flow through its admitted Plan and output Fore");
        }
        return Ok(());
    }
    if !request.authorize_capture {
        return Err("physical proof requires --authorize-capture".into());
    }
    let discovery = AlsaMicrophoneDiscovery::inspect(&request.arecord_executable)?;
    let matches = discovery
        .observations
        .iter()
        .filter(|item| item.card_id == request.card_id && item.device == request.device)
        .cloned()
        .collect::<Vec<_>>();
    let [selected] = matches.as_slice() else {
        return Err("microphone card/device does not select exactly one current endpoint".into());
    };
    let microphone = discovery.initialize_with_format(
        selected,
        MicrophoneLimits {
            capture_milliseconds: request.capture_milliseconds,
            timeout: Duration::from_secs(request.timeout_seconds),
        },
        if request.alsa_plug {
            AlsaMicrophoneFormat::AlsaPlug
        } else {
            AlsaMicrophoneFormat::ExactHardware
        },
    )?;
    let receipt = conduit_std_host::microphone_flow_proof::run(
        conduit_std_host::StdHostConfig {
            host_id: "host/microphone-flow-proof".into(),
            boot_id: "boot/microphone-flow-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        conduit_std_host::StdHostComposition::reference(),
        microphone,
        request.block_frames,
    )?;
    if opts.json {
        println!(
            "{}",
            serde_json::json!({"schema":"conduit.tools/microphone-flow-proof@1","proof_class":"explicit-finite-microphone-plan-play","receipt":receipt})
        );
    } else if !opts.quiet {
        println!(
            "MICROPHONE FLOW PROVED: plan={} blocks={} frames={} first-delivery-us={}",
            receipt.plan_id,
            receipt.blocks,
            receipt.total_frames,
            receipt.first_delivery_microseconds
        );
    }
    Ok(())
}
